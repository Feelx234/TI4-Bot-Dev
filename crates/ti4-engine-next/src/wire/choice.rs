//! Choices: the single way any actor decides anything.
//!
//! Every decision in the game — which ability to resolve in an open window, which action to
//! take on your turn, how to vote — is the same shape: *the engine enumerates legal options,
//! and an actor picks one of them*. Nothing else is a decision point.
//!
//! That shape is what makes several guarantees hold at once:
//!
//! * the engine is authoritative, because options are generated, never accepted from outside;
//! * a bot or an LLM can only ever select a legal option, with no channel to invent one;
//! * decisions are recorded, so a game replays exactly from a seed plus its decision log;
//! * an actor sees only the [`Choice`] it is handed and the public facts in [`Observed`],
//!   never another player's hand.
//!
//! [`Decider`] is deliberately tiny. Human input, a scripted conformance test, a seeded
//! random smoke run, and a scored bot are all the same interface.
//!
//! Ported from the oracle's `engine/choice.py`. The oracle's `Option` is [`ChoiceOption`]
//! here, because a type called `Option` in scope alongside `std::option::Option` is a trap.

use std::collections::{BTreeMap, BTreeSet};

use crate::observed::{Observed, SeatObservation};
use std::sync::Arc;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ti4_model::id::{PlayerId, UnitTypeId};
use ti4_model::units::Unit;


/// The kind used by a declining option.
pub const DECLINE_KIND: &str = "decline";
/// The id used by a declining option.
pub const DECLINE_ID: &str = "decline";

/// One legal thing an actor may pick.
///
/// `id` must be stable for a given game state so decision logs replay faithfully.
#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChoiceOption {
    pub id: String,
    pub kind: String,
    pub label: String,
    /// Structured detail for whatever will apply this option. Excluded from equality, as in
    /// the oracle: two options with the same id are the same option, and a payload that
    /// differed would mean the id was not stable.
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub payload: BTreeMap<String, Value>,
    /// What taking this option would do, when its producer can compute that without mutation.
    ///
    /// Runtime analysis rather than replay identity: the stable id and payload remain the
    /// authoritative instruction applied by the engine. Skipping this field also keeps old
    /// serialized choices readable while [`crate::wire::preview::Outcome`] retains compile-time-only
    /// diagnostic reasons.
    #[serde(skip)]
    pub preview: Option<crate::wire::preview::Preview>,
    /// True when this is the only legal option and was auto-selected. For UX feedback only;
    /// does not affect game logic or replay.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_resolved: bool,
}

impl PartialEq for ChoiceOption {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.kind == other.kind && self.label == other.label
    }
}

impl ChoiceOption {
    #[must_use]
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            label: String::new(),
            payload: BTreeMap::new(),
            preview: None,
            auto_resolved: false,
        }
    }

    #[must_use]
    pub fn labelled(
        id: impl Into<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            ..Self::new(id, kind)
        }
    }

    /// The standard "do nothing" option.
    #[must_use]
    pub fn decline() -> Self {
        Self::labelled(DECLINE_ID, DECLINE_KIND, "Decline")
    }

    #[must_use]
    pub fn is_decline(&self) -> bool {
        self.kind == DECLINE_KIND
    }

    /// Attach structured detail. Does not affect equality or the option's identity.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }

    /// Attach the `planet` (and, when known, `system`) payload a map UI uses to locate a planet
    /// answer. Keys already present are left as they are; like [`Self::with`], identity is
    /// unaffected.
    #[must_use]
    pub fn with_planet(mut self, planet: &str, system: Option<&str>) -> Self {
        self.payload
            .entry("planet".to_owned())
            .or_insert_with(|| Value::from(planet));
        if let Some(system) = system {
            self.payload
                .entry("system".to_owned())
                .or_insert_with(|| Value::from(system));
        }
        self
    }

    /// [`Self::with_planet`], looking the planet's system up with [`crate::ties::planets::system_of`].
    #[must_use]
    pub fn with_planet_located(
        self,
        state: &ti4_model::state::GameState,
        content: &ti4_content::ContentStore,
        sources: ti4_model::content_types::SourceSet,
        planet: &str,
    ) -> Self {
        let system = crate::ties::planets::system_of(state, content, sources, planet);
        self.with_planet(planet, system.as_ref().map(ti4_model::id::SystemId::as_str))
    }

    /// Attach an analytic consequence summary without changing this option's identity.
    #[must_use]
    pub fn previewed(mut self, preview: crate::wire::preview::Preview) -> Self {
        self.preview = Some(preview);
        self
    }

    /// What to show: the label if there is one, else the id.
    #[must_use]
    pub fn display(&self) -> &str {
        if self.label.is_empty() {
            &self.id
        } else {
            &self.label
        }
    }
}

/// A decision point handed to exactly one actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Choice {
    pub player: PlayerId,
    pub prompt: String,
    pub options: Vec<ChoiceOption>,
    /// Why this question exists and what remains outstanding in its transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<crate::wire::decision_context::DecisionContext>,
    /// Facts a client needs to present the question (current pools, who played a card, tokens
    /// left). Display only: never read by the engine, never copied into a [`DecisionRecord`],
    /// and skipped when empty, so records, replays and fingerprints are unaffected.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub details: serde_json::Map<String, Value>,
    /// An id prefix that names answers built by the asker rather than listed in `options`
    /// (a transaction's free-form deal). Unlisted ids starting with it are accepted as answers;
    /// whoever resolves the answer parses the rest and rejects what it cannot read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_form: Option<String>,
}

/// Display only: the header of an offer card: what it is, its kind, when it applies and the
/// printed or summarised effect. See [`Choice::offered`].
#[must_use]
pub fn offer_card(title: &str, tag: &str, window: Option<&str>, text: Option<&str>) -> Value {
    serde_json::json!({ "title": title, "tag": tag, "window": window, "text": text })
}

/// Display only: one labelled fact of an offer card, shown as text.
#[must_use]
pub fn offer_fact(label: &str, value: impl Into<Value>) -> Value {
    serde_json::json!({ "label": label, "value": value.into() })
}

/// Display only: a fact whose value is a unit type (shown with its name and icon).
#[must_use]
pub fn offer_fact_unit(label: &str, unit: &str) -> Value {
    serde_json::json!({ "label": label, "unit": unit })
}

/// Display only: a fact whose value is a planet (shown by its name, with its system).
#[must_use]
pub fn offer_fact_planet(label: &str, planet: &str, system: &str) -> Value {
    serde_json::json!({ "label": label, "planet": planet, "system": system })
}

/// Display only: a fact whose value is a system (the client names it by its planets).
#[must_use]
pub fn offer_fact_system(label: &str, system: &str) -> Value {
    serde_json::json!({ "label": label, "system": system })
}

/// Display only: a fact whose value is a seat (shown by the player's name).
#[must_use]
pub fn offer_fact_seat(label: &str, seat: &str) -> Value {
    serde_json::json!({ "label": label, "seat": seat })
}

/// Display only: a fact whose value is a technology (shown by its name).
#[must_use]
pub fn offer_fact_technology(label: &str, technology: &str) -> Value {
    serde_json::json!({ "label": label, "technology": technology })
}

/// Display only: a number that changes, e.g. commodities `1 -> 2` of at most `of`.
#[must_use]
pub fn offer_fact_change(label: &str, from: i64, to: i64, of: Option<i64>) -> Value {
    serde_json::json!({ "label": label, "from": from, "to": to, "of": of })
}

/// Display only: the button caption for an option and an optional hint under it.
#[must_use]
pub fn offer_caption(label: &str, hint: Option<&str>) -> Value {
    serde_json::json!({ "label": label, "hint": hint })
}

/// Display only: [`offer_caption`] for an option about another seat's unit: the client adds the
/// seat's name beside the hint.
#[must_use]
pub fn offer_caption_for_seat(label: &str, hint: Option<&str>, seat: &str) -> Value {
    serde_json::json!({ "label": label, "hint": hint, "seat": seat })
}

/// The `details` an offer carries (`kind`, `card`, `facts`, `captions`): what [`Choice::offered`] adds, for a producer
/// that completes the details of a question it did not build (the core dual's round agent question).
#[must_use]
pub fn offer_details(card: Value, facts: Vec<Value>, captions: &[(&str, Value)]) -> serde_json::Map<String, Value> {
    let captions: serde_json::Map<String, Value> = captions.iter().map(|(id, caption)| ((*id).to_owned(), caption.clone())).collect();
    let mut details = serde_json::Map::new();
    details.insert("kind".to_owned(), Value::String("offer".to_owned()));
    details.insert("card".to_owned(), card);
    details.insert("facts".to_owned(), Value::Array(facts));
    details.insert("captions".to_owned(), Value::Object(captions));
    details
}

impl Choice {
    #[must_use]
    pub fn new(player: PlayerId, prompt: impl Into<String>, options: Vec<ChoiceOption>) -> Self {
        Self {
            player,
            prompt: prompt.into(),
            options,
            context: None,
            details: serde_json::Map::new(),
            free_form: None,
        }
    }

    /// Accept answers whose id starts with `prefix` although they are not listed; see
    /// [`Choice::free_form`]. Also tells clients through `details.free_form`.
    #[must_use]
    pub fn accepting_free_form(mut self, prefix: &str) -> Self {
        self.details
            .insert("free_form".to_owned(), Value::String(prefix.to_owned()));
        self.free_form = Some(prefix.to_owned());
        self
    }

    /// The option an answer id stands for: the listed one, or a free-form one the asker built.
    #[must_use]
    pub fn answer_for(&self, option_id: &str) -> Option<ChoiceOption> {
        if let Some(listed) = self.option(option_id) {
            return Some(listed.clone());
        }
        // A movement answer that names one of the routes `details.routes` lists for an option: the
        // option itself, under the id of the route (rules version 29, `tactical::route_suffix`).
        if let Some((base, number)) = crate::ties::tactical::route_suffix(option_id)
            && number < crate::ties::tactical::route_count(self, base)
            && let Some(listed) = self.option(base)
        {
            let mut routed = listed.clone();
            routed.id = option_id.to_owned();
            return Some(routed);
        }
        let prefix = self.free_form.as_deref()?;
        (option_id.starts_with(prefix) && option_id.len() > prefix.len())
            .then(|| ChoiceOption::labelled(option_id, "offer", option_id))
    }

    /// Attach one display-only fact for clients; see [`Choice::details`].
    #[must_use]
    pub fn detailed(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }

    /// Present the question as an offer card for clients (display only, like [`Choice::detailed`]):
    /// `card` names what is asked (see [`offer_card`]), `facts` is a list of [`offer_fact`]s and
    /// `captions` maps option ids to [`offer_caption`]s that say what each answer does. Option ids,
    /// order and kinds are untouched, so decision records and replays are unaffected.
    #[must_use]
    pub fn offered(mut self, card: Value, facts: Vec<Value>, captions: &[(&str, Value)]) -> Self {
        self.details.extend(offer_details(card, facts, captions));
        self
    }

    /// Attach producer-authored typed semantics to this decision.
    #[must_use]
    pub fn contextualized(mut self, context: crate::wire::decision_context::DecisionContext) -> Self {
        debug_assert_eq!(
            self.player, context.actor,
            "a decision context must describe the seat receiving the choice"
        );
        self.context = Some(context);
        self
    }

    #[must_use]
    pub fn option(&self, option_id: &str) -> Option<&ChoiceOption> {
        self.options.iter().find(|o| o.id == option_id)
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.options.iter().map(|o| o.id.as_str()).collect()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.options.is_empty()
    }
}

/// A decider returned something that was not on offer.
///
/// This is the boundary that keeps an LLM or a buggy bot from inventing moves.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IllegalChoice {
    #[error("{player} chose {chosen:?}, which was not offered: {offered:?}")]
    NotOffered {
        player: PlayerId,
        chosen: String,
        offered: Vec<String>,
    },
    #[error("script wanted {wanted:?} but {player} was offered {offered:?}")]
    ScriptDiverged {
        player: PlayerId,
        wanted: String,
        offered: Vec<String>,
    },
    #[error("{player} was asked {prompt:?} with no options")]
    NoOptions { player: PlayerId, prompt: String },
    #[error("decider for {player} failed while answering {prompt:?}: {reason}")]
    DeciderFailed {
        player: PlayerId,
        prompt: String,
        reason: String,
    },
    #[error(
        "{player} was asked {prompt:?} {repeats} times in one engine step without progress (limit {MAX_IDENTICAL_ASKS_PER_STEP}); the decision cycle never ends"
    )]
    NoProgress {
        player: PlayerId,
        prompt: String,
        repeats: u32,
    },
}

/// The most times one identical choice (same seat, prompt and option ids) may be put within a
/// single engine step before the table declares the decision cycle stuck.
pub const MAX_IDENTICAL_ASKS_PER_STEP: u32 = 500;

/// Stable 64-bit FNV-1a over the seat, prompt and ordered option ids (length-prefixed, so
/// boundaries cannot blur). Written out rather than `DefaultHasher` so it never varies by run.
fn choice_fingerprint(choice: &Choice) -> u64 {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = BASIS;
    let mut feed = |text: &str| {
        for byte in (text.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(text.bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    feed(choice.player.as_str());
    feed(&choice.prompt);
    for option in &choice.options {
        feed(&option.id);
    }
    hash
}

/// Reject any answer that was not among the offered options.
///
/// # Errors
/// [`IllegalChoice::NotOffered`].
pub fn validate(choice: &Choice, option: ChoiceOption) -> Result<ChoiceOption, IllegalChoice> {
    if choice.option(&option.id).is_some() {
        return Ok(option);
    }
    if let Some(built) = choice.answer_for(&option.id) {
        return Ok(built);
    }
    Err(IllegalChoice::NotOffered {
        player: choice.player.clone(),
        chosen: option.id,
        offered: choice.ids().into_iter().map(str::to_owned).collect(),
    })
}


/// Anything that can answer a [`Choice`].
///
/// `&mut self` because a decider may carry state — a script position, an RNG stream.
///
/// # Example
///
/// ```
/// use ti4_engine::wire::choice::{Decider, Choice, ChoiceOption, IllegalChoice};
///
/// struct AlwaysDecline;
///
/// impl Decider for AlwaysDecline {
///     fn choose(&mut self, _choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
///         Ok(ChoiceOption::decline())
///     }
/// }
/// ```
pub trait Decider {
    /// # Errors
    /// [`IllegalChoice`] if the decider cannot answer, e.g. an exhausted script whose next
    /// wanted option was not offered.
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice>;

    /// Answer a choice with the position in hand.
    ///
    /// The view is a [`SeatObservation`] bound to this choice's owner by the engine: public facts
    /// come through its deref to [`Observed`], and held-secret progress is reachable only for the
    /// bound seat — there is no argument an opponent could be requested with. A decider that does
    /// not read secrets simply ignores the binding.
    ///
    /// Defaulted to [`Decider::choose`], so a scripted test or a random smoke run needs to know
    /// nothing about the board, and a scorer overrides only this one. The engine calls this at
    /// every site that has a position to offer, and calls `choose` at the rest — a window that
    /// owns a slice of the game rather than the whole of it cannot honestly produce one.
    ///
    /// # Errors
    /// As [`Decider::choose`].
    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let _ = seen;
        self.choose(choice)
    }

    /// The policy's `(score, probability)` per option of the choice about to be asked, in
    /// `choice.options` order, handed down by a scoring wrapper before it delegates.
    ///
    /// Presentation only: a decider that answers in the policy's place (a manual seat) shows them
    /// to the person choosing. The default ignores them, and nothing may let them change an answer.
    fn stage_scores(&mut self, scores: Vec<(Option<f64>, Option<f64>)>) {
        let _ = scores;
    }
}

/// Always take the first option. Deterministic; the default in tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstOption;

impl Decider for FirstOption {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        choice
            .options
            .first()
            .cloned()
            .ok_or_else(|| IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            })
    }
}

/// Decline whenever declining is legal, else take the first option.
#[derive(Debug, Clone, Copy, Default)]
pub struct AlwaysDecline;

impl Decider for AlwaysDecline {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        choice
            .options
            .iter()
            .find(|o| o.is_decline())
            .cloned()
            .map_or_else(|| FirstOption.choose(choice), Ok)
    }
}

/// Answer from a fixed sequence of option ids, falling back once exhausted.
///
/// The workhorse for conformance tests: it states the exact line of play a scenario is
/// asserting, and fails loudly if the engine offers something unexpected.
pub struct Scripted {
    queue: std::collections::VecDeque<String>,
    fallback: Box<dyn Decider>,
}

impl Scripted {
    #[must_use]
    pub fn new<I, S>(option_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::with_fallback(option_ids, Box::new(FirstOption))
    }

    #[must_use]
    pub fn with_fallback<I, S>(option_ids: I, fallback: Box<dyn Decider>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            queue: option_ids.into_iter().map(Into::into).collect(),
            fallback,
        }
    }

    /// How many scripted answers remain.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.queue.len()
    }
}

impl Decider for Scripted {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let Some(wanted) = self.queue.pop_front() else {
            return self.fallback.choose(choice);
        };
        choice
            .answer_for(&wanted)
            .ok_or_else(|| IllegalChoice::ScriptDiverged {
                player: choice.player.clone(),
                wanted,
                offered: choice.ids().into_iter().map(str::to_owned).collect(),
            })
    }
}

/// Records every `Choice` it is asked, context included, before delegating the answer to an
/// inner decider.
///
/// For tests that need to see a producer's typed context and not only its answer: `Scripted`
/// and `FirstOption` answer without keeping what they were shown, and re-deriving a `Choice`
/// outside the engine to inspect its context would test a second copy of the construction
/// rather than the one a policy actually receives.
pub struct Capturing {
    seen: std::rc::Rc<std::cell::RefCell<Vec<Choice>>>,
    inner: Box<dyn Decider>,
}

impl Capturing {
    #[must_use]
    pub fn new(inner: Box<dyn Decider>) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<Choice>>>) {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        (
            Self {
                seen: seen.clone(),
                inner,
            },
            seen,
        )
    }
}

impl Decider for Capturing {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        self.inner.choose(choice)
    }
}

/// Uniform random over legal options from a seed.
///
/// Not a bot. This is what drives bot-versus-bot smoke runs that assert every game
/// terminates and no illegal state is reachable.
///
/// **The stream is not the oracle's.** The oracle uses Python's Mersenne Twister via
/// `random.Random(seed)`; this uses `ChaCha8`, which is reproducible across platforms and
/// Rust versions in a way Python's is not. The same seed therefore plays a *different* legal
/// game. Reproducing an oracle game needs its decision log replayed through [`Scripted`],
/// or the legacy entropy translator planned in M03-007.
///
/// A clone continues the same stream from the same position: with [`Game::fork`] it is what a
/// checkpoint of a game with random seats keeps.
///
/// [`Game::fork`]: crate::game::Game::fork
#[derive(Clone)]
pub struct SeededRandom {
    rng: ChaCha8Rng,
}

impl SeededRandom {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl Decider for SeededRandom {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if choice.options.is_empty() {
            return Err(IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            });
        }
        let index = self.rng.random_range(0..choice.options.len());
        Ok(choice.options[index].clone())
    }
}

/// One resolved choice, for replay (determinism) and for explaining bot play.
///
/// A shared, immutable handle: cloning a record (and so a whole [`DecisionLog`]) copies one pointer
/// instead of its strings and option list. The fields are reached through `Deref`
/// ([`DecisionRecordFields`]); a changed record is built with [`DecisionRecord::new`] or
/// by assigning through `DerefMut` (which copies a shared record first). Serialises exactly as the plain struct of those fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecord(Arc<DecisionRecordFields>);

/// The fields of a [`DecisionRecord`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionRecordFields {
    pub player: PlayerId,
    pub prompt: String,
    pub chosen: String,
    pub offered: Vec<String>,
    /// Why the engine asked, when the producer supplies it (OBS-003a).
    ///
    /// Optional and skipped when absent, so a record written before contexts existed serialises to
    /// exactly the bytes it always did. That is what keeps the V1 fingerprint of an old replay
    /// stable while V2 can bind the context; see `fingerprint::decision_hash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<crate::wire::decision_context::DecisionContext>,
}

impl std::ops::Deref for DecisionRecord {
    type Target = DecisionRecordFields;
    fn deref(&self) -> &DecisionRecordFields {
        &self.0
    }
}

impl std::ops::DerefMut for DecisionRecord {
    /// Copies the fields first when the record is shared.
    fn deref_mut(&mut self) -> &mut DecisionRecordFields {
        Arc::make_mut(&mut self.0)
    }
}

impl Serialize for DecisionRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DecisionRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DecisionRecordFields::deserialize(deserializer).map(|fields| Self(Arc::new(fields)))
    }
}

impl From<DecisionRecordFields> for DecisionRecord {
    fn from(fields: DecisionRecordFields) -> Self {
        Self(Arc::new(fields))
    }
}

impl DecisionRecord {
    #[must_use]
    pub fn new(
        player: PlayerId,
        prompt: String,
        chosen: String,
        offered: Vec<String>,
        context: Option<crate::wire::decision_context::DecisionContext>,
    ) -> Self {
        Self(Arc::new(DecisionRecordFields {
            player,
            prompt,
            chosen,
            offered,
            context,
        }))
    }

    /// Whether both handles are the same record (not merely equal ones).
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The same record with no context, which is what the V1 fingerprint contract hashes.
    #[must_use]
    pub fn without_context(&self) -> Self {
        if self.context.is_none() {
            return self.clone();
        }
        Self::new(
            self.player.clone(),
            self.prompt.clone(),
            self.chosen.clone(),
            self.offered.clone(),
            None,
        )
    }
}

/// Ordered record of every choice made, sufficient to replay a game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLog {
    pub records: Vec<DecisionRecord>,
}

impl DecisionLog {
    pub fn record(&mut self, choice: &Choice, option: &ChoiceOption) {
        self.records.push(DecisionRecord::new(
            choice.player.clone(),
            choice.prompt.clone(),
            option.id.clone(),
            choice.ids().into_iter().map(str::to_owned).collect(),
            // The trigger is display metadata derived from the event; a record and its fingerprint
            // hold only what a replay needs.
            choice
                .context
                .as_ref()
                .map(crate::wire::decision_context::DecisionContext::without_display_fields),
        ));
    }

    /// Replay script — the chosen option ids, optionally for one player.
    #[must_use]
    pub fn as_script(&self, player: Option<&PlayerId>) -> Vec<String> {
        self.records
            .iter()
            .filter(|r| player.is_none_or(|p| &r.player == p))
            .map(|r| r.chosen.clone())
            .collect()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Deciders by player, with a default for anyone unassigned.
///
/// # Example
///
/// ```
/// use ti4_engine::wire::choice::{Table, Decider, Scripted};
/// use ti4_model::id::PlayerId;
///
/// let mut table = Table::with_default(Box::new(Scripted::new(vec![String::new()])));
/// table.seat(PlayerId::new("a"), Box::new(Scripted::new(vec!["first".to_owned()])));
/// ```
type ObservedOffer = Box<dyn FnMut(&[DecisionRecord], &ti4_model::state::GameState) + Send>;

/// Told about each decision the engine settles without asking.
type AutoResolvedObserver = Box<dyn FnMut(&AutoResolved) + Send>;

pub struct Table {
    deciders: BTreeMap<PlayerId, Box<dyn Decider>>,
    default: Box<dyn Decider>,
    pub log: DecisionLog,
    observed_offer: Option<ObservedOffer>,
    auto_resolved_observer: Option<AutoResolvedObserver>,
    window_observer: Option<WindowObserver>,
    /// Decisions settled without asking, since the last drain. Never part of the decision log.
    auto_resolved: Vec<AutoResolved>,
    choice_failures: u64,
    last_choice_error: Option<IllegalChoice>,
    /// Identical-choice counts for the current engine step, keyed by [`choice_fingerprint`].
    asks_this_step: BTreeMap<u64, u32>,
}

/// Callback told that a reaction window opened for a seat that is not asked anything in it.
///
/// Arguments: the seat, the window's event type, whether it is a "when" or "after" window, the
/// current state, and a function that publishes the decisions settled so far. The callback is
/// presentation only: it gets no way to change the game, answer a choice, or draw from any random
/// stream, and nothing it does is journaled.
pub type WindowObserver = Box<
    dyn FnMut(
            &PlayerId,
            &str,
            crate::ties::timing::Relation,
            &ti4_model::state::GameState,
            &mut dyn FnMut(),
        ) + Send,
>;

/// A decision the engine settled itself because exactly one option was legal.
///
/// Public feedback only. It is deliberately *not* a [`DecisionRecord`]: the skipped ask is
/// never journaled, so a replay re-derives the same lone option and the log is unchanged.
/// It carries only what the actor was already shown (the prompt and the option's label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoResolved {
    pub player: PlayerId,
    pub prompt: String,
    pub option_id: String,
    pub label: String,
    /// Why there was nothing to decide, in a short sentence.
    pub reason: String,
}

/// The state-carrying part of a [`Table`] (see [`Table::state`]). `Send`, unlike a table.
#[derive(Debug, Clone)]
pub struct TableState {
    pub log: DecisionLog,
    auto_resolved: Vec<AutoResolved>,
    choice_failures: u64,
    last_choice_error: Option<IllegalChoice>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            deciders: BTreeMap::new(),
            default: Box::new(FirstOption),
            log: DecisionLog::default(),
            observed_offer: None,
            auto_resolved_observer: None,
            window_observer: None,
            auto_resolved: Vec::new(),
            choice_failures: 0,
            last_choice_error: None,
            asks_this_step: BTreeMap::new(),
        }
    }
}

impl Table {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything about this table that is game state rather than input wiring: the decision
    /// log and the failure counters. Deciders and observation callbacks are NOT part of it.
    ///
    /// FORK RULE: destructured without `..`; a new `Table` field must be copied here or listed
    /// as input wiring.
    #[must_use]
    pub fn state(&self) -> TableState {
        self.state_with(true)
    }

    /// [`Table::state`] without copying the decision log (it is left empty). For a snapshot
    /// whose holder already has the log and puts it back before stepping.
    #[must_use]
    pub fn state_unlogged(&self) -> TableState {
        self.state_with(false)
    }

    fn state_with(&self, keep_log: bool) -> TableState {
        let Self {
            deciders: _, // input wiring: whoever answers is chosen by the caller
            default: _,  // input wiring
            log,
            observed_offer: _,         // input wiring: session callbacks
            auto_resolved_observer: _, // input wiring
            // Bluff hook (skipped-window observer): input wiring, intentionally NOT copied, so a
            // forked batch simulation or a replay never fires bluff holds (AGENTS.md FORK RULE).
            window_observer: _,
            auto_resolved,
            choice_failures,
            last_choice_error,
            asks_this_step: _, // per-step scratch, cleared by `begin_step`; not game state
        } = self;
        TableState {
            log: if keep_log {
                log.clone()
            } else {
                DecisionLog::default()
            },
            auto_resolved: auto_resolved.clone(),
            choice_failures: *choice_failures,
            last_choice_error: last_choice_error.clone(),
        }
    }

    /// Take over `state` (the log and counters) while keeping this table's deciders and
    /// callbacks.
    pub fn restore_state(&mut self, state: TableState) {
        let TableState {
            log,
            auto_resolved,
            choice_failures,
            last_choice_error,
        } = state;
        self.log = log;
        self.auto_resolved = auto_resolved;
        self.choice_failures = choice_failures;
        self.last_choice_error = last_choice_error;
    }

    /// A table with first-option deciders, no callbacks, and the given state.
    #[must_use]
    pub fn from_state(state: TableState) -> Self {
        let TableState {
            log,
            auto_resolved,
            choice_failures,
            last_choice_error,
        } = state;
        Self {
            log,
            auto_resolved,
            choice_failures,
            last_choice_error,
            ..Self::default()
        }
    }

    /// A table where everyone unassigned uses this decider.
    #[must_use]
    pub fn with_default(default: Box<dyn Decider>) -> Self {
        Self {
            default,
            ..Self::default()
        }
    }

    /// Replace the decider that answers for every unseated player (the log is kept).
    pub fn set_default(&mut self, decider: Box<dyn Decider>) {
        self.default = decider;
    }

    pub fn seat(&mut self, player: PlayerId, decider: Box<dyn Decider>) {
        self.deciders.insert(player, decider);
    }

    /// Notify a session when a nested offer is reached, before its decider blocks.
    pub fn on_observed_offer(
        &mut self,
        callback: impl FnMut(&[DecisionRecord], &ti4_model::state::GameState) + Send + 'static,
    ) {
        self.observed_offer = Some(Box::new(callback));
    }

    /// Be told when a reaction window opens for a seat that has nothing to decide in it.
    pub fn on_window_skipped(&mut self, observer: WindowObserver) {
        self.window_observer = Some(observer);
    }

    /// Tell the observer (if any) that `player` is not asked in this window.
    ///
    /// Purely informational: the answer sequence, the log and the game state are untouched.
    pub fn window_skipped(
        &mut self,
        player: &PlayerId,
        event_type: &str,
        relation: crate::ties::timing::Relation,
        state: &ti4_model::state::GameState,
    ) {
        let Some(mut observer) = self.window_observer.take() else {
            return;
        };
        let records = &self.log.records;
        let offered = &mut self.observed_offer;
        observer(player, event_type, relation, state, &mut || {
            if let Some(callback) = offered {
                callback(records, state);
            }
        });
        self.window_observer = Some(observer);
    }

    /// Be told, as it happens, about each decision the engine settles without asking.
    pub fn on_auto_resolved(&mut self, callback: impl FnMut(&AutoResolved) + Send + 'static) {
        self.auto_resolved_observer = Some(Box::new(callback));
    }

    /// Settle a choice that has exactly one option without asking anyone.
    ///
    /// Returns the option (flagged `auto_resolved`) and leaves a note for the actor's client.
    /// Nothing enters the decision log, matching how these skips always behaved. `None` when
    /// the choice has any other number of options, so the caller asks as usual.
    pub fn auto_resolve(&mut self, choice: &Choice, reason: &str) -> Option<ChoiceOption> {
        let option = auto_resolve_single(&choice.options)?;
        let note = AutoResolved {
            player: choice.player.clone(),
            prompt: choice.prompt.clone(),
            option_id: option.id.clone(),
            label: if option.label.is_empty() {
                option.id.clone()
            } else {
                option.label.clone()
            },
            reason: reason.to_owned(),
        };
        if let Some(observer) = &mut self.auto_resolved_observer {
            observer(&note);
        }
        // Bounded: simulations never drain it.
        if self.auto_resolved.len() >= 64 {
            self.auto_resolved.remove(0);
        }
        self.auto_resolved.push(note);
        Some(option)
    }

    /// Take the notes left by [`Table::auto_resolve`] since the last call.
    pub fn take_auto_resolved(&mut self) -> Vec<AutoResolved> {
        std::mem::take(&mut self.auto_resolved)
    }

    /// Start a new engine step: forget how often each choice was asked.
    pub fn begin_step(&mut self) {
        self.asks_this_step.clear();
    }

    /// Count this ask; `Err(NoProgress)` once the identical choice exceeds the per-step limit.
    fn count_ask(&mut self, choice: &Choice) -> Result<(), IllegalChoice> {
        let seen = self
            .asks_this_step
            .entry(choice_fingerprint(choice))
            .or_insert(0);
        *seen = seen.saturating_add(1);
        // Ten times past the limit, the refusal is being swallowed and the choice re-asked: some
        // effect turned the error into "no answer" and loops. Spinning on refusals is still a hang,
        // so stop the process with the decision named rather than run on silently.
        assert!(
            *seen <= MAX_IDENTICAL_ASKS_PER_STEP.saturating_mul(10),
            "{} was asked {:?} {} times in one engine step: a decision cycle is ignoring the NoProgress refusal",
            choice.player,
            choice.prompt,
            *seen
        );
        if *seen > MAX_IDENTICAL_ASKS_PER_STEP {
            let outcome = Err(IllegalChoice::NoProgress {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                repeats: *seen,
            });
            self.remember_choice_error(&outcome);
            return outcome.map(|_: ChoiceOption| ());
        }
        Ok(())
    }

    /// Put a choice to its actor, validate the answer, and record it.
    ///
    /// # Errors
    /// [`IllegalChoice`] if the answer was not on offer — the boundary that stops a bot
    /// inventing a move.
    pub fn ask(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.count_ask(choice)?;
        let decider = self
            .deciders
            .get_mut(&choice.player)
            .unwrap_or(&mut self.default);
        let answer = decider.choose(choice);
        let outcome = answer.and_then(|answer| self.settle(choice, answer));
        self.remember_choice_error(&outcome);
        outcome
    }

    /// Put a choice to its actor along with the public position.
    ///
    /// Identical to [`Table::ask`] except for what the decider is shown, and the answer goes
    /// through the same validation and the same log — so a game driven through this path replays
    /// through the other one.
    ///
    /// # Errors
    /// As [`Table::ask`].
    pub fn ask_seeing(
        &mut self,
        choice: &Choice,
        seen: &Observed<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.count_ask(choice)?;
        let mut associated = choice.clone();
        if let Some(context) = associated.context.take() {
            associated.context = Some(context.about_invasion(seen.state));
        }
        // A decision raised by an agenda being revealed or resolved (Veto, a rider, Confusing
        // Legal Text, any reaction window on it) is about that card: it carries the printed agenda
        // as display-only `details.agenda`, exactly as vote decisions do, unless the producer
        // attached it already.
        let trigger_agenda = associated
            .context
            .as_ref()
            .and_then(|context| context.trigger.as_ref())
            .and_then(|trigger| trigger.agenda.clone());
        if let Some(agenda) = trigger_agenda {
            if !associated.details.contains_key("agenda") {
                associated.details.insert(
                    "agenda".to_owned(),
                    crate::ties::strategy_cards::agenda_details(seen.content, &agenda),
                );
            }
        }
        let choice = &associated;
        if let Some(callback) = &mut self.observed_offer {
            callback(&self.log.records, seen.state);
        }
        let decider = self
            .deciders
            .get_mut(&choice.player)
            .unwrap_or(&mut self.default);
        // The private observation is bound here, inside the engine: the decider was just looked
        // up by `choice.player`, so the view it receives answers for exactly that seat. Policy-
        // side code never sees a constructor for this type.
        let seat_view = SeatObservation::bind(seen, choice.player.clone());
        let answer = decider.choose_seeing(choice, &seat_view);
        let outcome = answer.and_then(|answer| self.settle(choice, answer));
        self.remember_choice_error(&outcome);
        outcome
    }

    // Strict callers can detect errors swallowed by legacy Option-returning effect APIs.
    pub fn choice_error_checkpoint(&self) -> u64 {
        self.choice_failures
    }

    pub fn choice_error_since(&self, checkpoint: u64) -> Option<IllegalChoice> {
        (self.choice_failures != checkpoint)
            .then(|| self.last_choice_error.clone())
            .flatten()
    }

    fn remember_choice_error(&mut self, outcome: &Result<ChoiceOption, IllegalChoice>) {
        if let Err(error) = outcome {
            self.choice_failures = self.choice_failures.saturating_add(1);
            self.last_choice_error = Some(error.clone());
        }
    }

    /// Validate an answer and record it. Shared, so the two ask paths cannot drift.
    fn settle(
        &mut self,
        choice: &Choice,
        answer: ChoiceOption,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let option = validate(choice, answer)?;
        self.log.record(choice, &option);
        Ok(option)
    }
}

// ─── building options ──────────────────────────────────────────────────────────

/// Build options from `(id, label)` pairs.
#[must_use]
pub fn options_from<'a, I>(items: I, kind: &str) -> Vec<ChoiceOption>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    items
        .into_iter()
        .map(|(id, label)| ChoiceOption::labelled(id, kind, label))
        .collect()
}

/// Auto-resolves single-option choices.
///
/// When exactly one legal option exists, returns it with `auto_resolved = true` set.
/// Otherwise returns `None`, allowing the normal decision flow to proceed.
///
/// Used to skip meaningless decisions where the player has no real choice, improving UX
/// by automatically selecting the only option and showing a non-blocking toast notification.
#[must_use]
pub fn auto_resolve_single(options: &[ChoiceOption]) -> Option<ChoiceOption> {
    if options.len() == 1 {
        let mut option = options[0].clone();
        option.auto_resolved = true;
        Some(option)
    } else {
        None
    }
}

/// The first index of each distinct item, keeping the original order.
///
/// The shape behind every duplicate-option fix in the engine: build options from
/// *distinguishable* things rather than from every thing, keeping the first index so the
/// option id still points at a real element.
#[must_use]
pub fn first_of_each<T, K, F>(items: &[T], key: F) -> Vec<(usize, &T)>
where
    K: Ord,
    F: Fn(&T) -> K,
{
    let mut seen = BTreeSet::new();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| seen.insert(key(item)))
        .collect()
}

/// How a unit is distinguished when offering it as an option.
///
/// Owner is not in the key: every caller is choosing among one player's own units.
pub type UnitKey = (UnitTypeId, bool);

/// The first index of each *distinguishable* unit, keyed by type and damage.
///
/// A unit is its type, its owner, and whether it has taken damage. Units matching on those
/// are interchangeable, so offering one option each is offering the same move several times
/// — and that is not merely verbose. **Deciders weigh options one by one**, and a sampling
/// bot draws from the option list, so a move written five times drew five times the
/// probability of an equally good move written once. In the oracle a player holding five
/// fighters and one dreadnought assigned its hits to a fighter five times in six no matter
/// what its scoring thought of the trade, because the count decided rather than the score.
#[must_use]
pub fn distinct_units(units: &[Unit]) -> Vec<(usize, &Unit)> {
    first_of_each(units, |u: &Unit| {
        (u.type_id.clone(), u.sustained_damage) as UnitKey
    })
}

/// `destroy dreadnought` / `destroy dreadnought (damaged)`.
///
/// Damage is shown rather than folded away: losing a ship that has already taken a hit is a
/// different proposition from losing a fresh one, and collapsing the two would hide a real
/// choice instead of removing a false one.
#[must_use]
pub fn unit_label(verb: &str, type_id: &UnitTypeId, damaged: bool) -> String {
    let suffix = if damaged { " (damaged)" } else { "" };
    format!("{verb} {type_id}{suffix}")
}


#[cfg(test)]
#[allow(dead_code, reason = "shared by the Observed tests of 8b3")]
pub(crate) mod planet_payload {
    use super::{Choice, ChoiceOption, Value};

    /// The option offered under `id`, or a panic naming what was offered.
    pub(crate) fn offered<'a>(choice: &'a Choice, id: &str) -> &'a ChoiceOption {
        choice
            .options
            .iter()
            .find(|option| option.id == id)
            .unwrap_or_else(|| panic!("{id} not offered; got {:?}", choice.ids()))
    }

    /// `option` names `planet` in `system`.
    pub(crate) fn assert_locates(option: &ChoiceOption, planet: &str, system: &str) {
        assert_eq!(
            option.payload.get("planet").and_then(Value::as_str),
            Some(planet),
            "planet payload of {}",
            option.id
        );
        assert_eq!(
            option.payload.get("system").and_then(Value::as_str),
            Some(system),
            "system payload of {}",
            option.id
        );
    }

    /// `option` is not a planet and says nothing about one.
    pub(crate) fn assert_not_a_planet(option: &ChoiceOption) {
        assert!(
            !option.payload.contains_key("planet"),
            "{} is not a planet but carries {:?}",
            option.id,
            option.payload
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_content::ContentStore;
    #[test]
    fn decision_records_serialise_as_plain_fields_and_clone_by_sharing() {
        let record = DecisionRecord::new(
            PlayerId::new("a"),
            "pick".to_owned(),
            "x".to_owned(),
            vec!["x".to_owned(), "y".to_owned()],
            None,
        );
        assert_eq!(
            serde_json::to_string(&record).unwrap(),
            r#"{"player":"a","prompt":"pick","chosen":"x","offered":["x","y"]}"#
        );
        let back: DecisionRecord =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(back, record);
        let mut copy = record.clone();
        assert!(copy.ptr_eq(&record));
        copy.chosen = "y".to_owned();
        assert!(!copy.ptr_eq(&record));
        assert_eq!(record.chosen, "x");
    }

    /// `with_planet` adds `planet`/`system`, keeps keys already set, and never touches identity.
    #[test]
    fn with_planet_adds_location_without_overwriting_or_changing_identity() {
        let plain = ChoiceOption::labelled("x", "planet", "X");
        let located = plain.clone().with_planet("lodor", Some("26"));
        assert_eq!(located, plain, "payload is not identity");
        planet_payload::assert_locates(&located, "lodor", "26");

        let kept = ChoiceOption::labelled("x", "planet", "X")
            .with("planet", "quann")
            .with("system", "25")
            .with_planet("lodor", Some("26"));
        planet_payload::assert_locates(&kept, "quann", "25");

        let nowhere = ChoiceOption::labelled("x", "planet", "X").with_planet("lodor", None);
        assert_eq!(
            nowhere.payload.get("planet").and_then(Value::as_str),
            Some("lodor")
        );
        assert!(
            !nowhere.payload.contains_key("system"),
            "no system invented"
        );
    }

    /// `with_planet_located` looks the system up; an unknown planet gets no `system`.
    #[test]
    fn with_planet_located_finds_the_printed_tile_and_invents_nothing() {
        let state = ti4_engine_legacy::fixtures::game(&["a"]);
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::POK;
        let lodor = ChoiceOption::labelled("lodor", "planet", "lodor")
            .with_planet_located(&state, content, sources, "lodor");
        planet_payload::assert_locates(&lodor, "lodor", "26");
        let ghost = ChoiceOption::labelled("ghost", "planet", "ghost").with_planet_located(
            &state,
            content,
            sources,
            "not_a_planet",
        );
        assert!(!ghost.payload.contains_key("system"));
    }

    #[test]
    fn obs008c1_context_records_while_runtime_preview_stays_out_of_replay_identity() {
        use crate::wire::decision_context::{
            ConstraintKind, DecisionContext, DecisionSource, OutstandingConstraint,
        };
        use crate::wire::preview::{Delta, Preview, Quantity};
        use ti4_model::state::Phase;

        let context = DecisionContext::new(
            pid("a"),
            DecisionSource::Rule("75.2".to_owned()),
            "pay_resources",
            Phase::Action,
            2,
        )
        .owing(OutstandingConstraint::new(ConstraintKind::Resources, 4, 1));
        let option =
            ChoiceOption::new("trade_good", "pay").previewed(Preview::certain(vec![Delta::new(
                Quantity::TradeGoods,
                2,
                1,
            )]));
        let encoded = serde_json::to_string(&option).expect("serialize option");
        assert!(
            !encoded.contains("preview"),
            "analysis is not replay identity"
        );
        let decoded: ChoiceOption = serde_json::from_str(&encoded).expect("old shape reads");
        assert!(decoded.preview.is_none());
        assert_eq!(option, decoded, "preview does not change option identity");

        let asked = Choice::new(pid("a"), "pay", vec![option]).contextualized(context.clone());
        let mut table = Table::new();
        table.ask(&asked).expect("first option is legal");
        let record = table.log.records.first().expect("recorded");
        assert_eq!(record.context.as_ref(), Some(&context));
        assert_eq!(
            crate::wire::fingerprint::decision_hash(crate::wire::fingerprint::CanonicalHashVersion::V1, record,),
            crate::wire::fingerprint::decision_hash(
                crate::wire::fingerprint::CanonicalHashVersion::V1,
                &record.without_context(),
            ),
            "V1 remains byte-compatible by stripping typed context"
        );
    }

    fn pid(id: &str) -> PlayerId {
        PlayerId::new(id)
    }

    fn unit(type_id: &str, damaged: bool) -> Unit {
        Unit {
            sustained_damage: damaged,
            ..Unit::new(UnitTypeId::new(type_id), pid("a"))
        }
    }

    fn choice(options: Vec<ChoiceOption>) -> Choice {
        Choice::new(pid("a"), "pick one", options)
    }

    fn three() -> Choice {
        choice(vec![
            ChoiceOption::labelled("x", "action", "Do X"),
            ChoiceOption::labelled("y", "action", "Do Y"),
            ChoiceOption::decline(),
        ])
    }

    // -- options ---------------------------------------------------------------

    #[test]
    fn an_option_is_identified_by_its_id_not_its_payload() {
        // A payload that changed equality would mean the id was not stable, which is
        // exactly what replay depends on.
        let bare = ChoiceOption::new("move", "movement");
        let loaded = ChoiceOption::new("move", "movement").with("from", "18");
        assert_eq!(bare, loaded);
        assert_ne!(bare.payload, loaded.payload);
    }

    #[test]
    fn an_option_displays_its_label_or_falls_back_to_its_id() {
        assert_eq!(ChoiceOption::labelled("x", "k", "Do X").display(), "Do X");
        assert_eq!(ChoiceOption::new("x", "k").display(), "x");
    }

    #[test]
    fn declining_is_recognised_by_kind_not_by_id() {
        assert!(ChoiceOption::decline().is_decline());
        assert!(!ChoiceOption::new("x", "action").is_decline());
        // A differently-named decline still counts.
        assert!(ChoiceOption::new("pass_window", DECLINE_KIND).is_decline());
    }

    #[test]
    fn auto_resolve_single_resolves_single_options() {
        let single = vec![ChoiceOption::labelled("only", "action", "Only Option")];
        let resolved = auto_resolve_single(&single).expect("single option");
        assert_eq!(resolved.id, "only");
        assert!(resolved.auto_resolved, "flag should be set");
    }

    #[test]
    fn table_auto_resolve_notes_the_lone_option_without_journaling_it() {
        let mut table = Table::new();
        let one = Choice::new(
            PlayerId::new("a"),
            "pay 1 more resources",
            vec![ChoiceOption::labelled("tg", "pay", "trade goods")],
        );
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        table.on_auto_resolved(move |n| sink.lock().unwrap().push(n.clone()));
        let option = table.auto_resolve(&one, "only way").expect("one option");
        assert!(option.auto_resolved);
        assert!(table.log.is_empty(), "journal must stay untouched");
        let notes = table.take_auto_resolved();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].label, "trade goods");
        assert_eq!(notes[0].reason, "only way");
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(table.take_auto_resolved().is_empty());

        let two = Choice::new(
            PlayerId::new("a"),
            "p",
            vec![
                ChoiceOption::labelled("x", "k", "X"),
                ChoiceOption::labelled("y", "k", "Y"),
            ],
        );
        assert!(table.auto_resolve(&two, "r").is_none());
        assert!(table.take_auto_resolved().is_empty());
    }

    #[test]
    fn auto_resolve_single_returns_none_for_multiple_options() {
        let multiple = vec![
            ChoiceOption::labelled("x", "action", "Do X"),
            ChoiceOption::labelled("y", "action", "Do Y"),
        ];
        assert!(auto_resolve_single(&multiple).is_none());
    }

    #[test]
    fn auto_resolve_single_returns_none_for_empty_options() {
        let empty: Vec<ChoiceOption> = vec![];
        assert!(auto_resolve_single(&empty).is_none());
    }

    #[test]
    fn a_choice_finds_its_options_by_id() {
        let c = three();
        assert_eq!(c.option("y").unwrap().label, "Do Y");
        assert!(c.option("nonesuch").is_none());
        assert_eq!(c.ids(), vec!["x", "y", "decline"]);
    }

    // -- validation ------------------------------------------------------------

    #[test]
    fn an_answer_that_was_not_offered_is_rejected() {
        // The boundary that keeps a bot or an LLM from inventing a move.
        let err = validate(&three(), ChoiceOption::new("invented", "action")).unwrap_err();
        assert!(
            matches!(err, IllegalChoice::NotOffered { ref chosen, .. } if chosen == "invented"),
            "{err}"
        );
    }

    #[test]
    fn an_answer_that_was_offered_passes_through() {
        let answer = validate(&three(), ChoiceOption::new("y", "action")).unwrap();
        assert_eq!(answer.id, "y");
    }

    #[test]
    fn the_rejection_names_what_was_on_offer() {
        let err = validate(&three(), ChoiceOption::new("z", "action")).unwrap_err();
        let message = err.to_string();
        assert!(message.contains('a'), "{message}");
        assert!(message.contains('z'), "{message}");
        assert!(message.contains("decline"), "{message}");
    }

    // -- deciders ---------------------------------------------------------------

    #[test]
    fn first_option_is_deterministic() {
        assert_eq!(FirstOption.choose(&three()).unwrap().id, "x");
        assert_eq!(FirstOption.choose(&three()).unwrap().id, "x");
    }

    #[test]
    fn always_decline_takes_the_decline_when_there_is_one() {
        assert_eq!(AlwaysDecline.choose(&three()).unwrap().id, "decline");
    }

    #[test]
    fn always_decline_falls_back_to_the_first_option() {
        let no_decline = choice(vec![
            ChoiceOption::new("x", "action"),
            ChoiceOption::new("y", "action"),
        ]);
        assert_eq!(AlwaysDecline.choose(&no_decline).unwrap().id, "x");
    }

    #[test]
    fn a_decider_asked_with_no_options_reports_it_rather_than_panicking() {
        let empty = choice(vec![]);
        assert!(matches!(
            FirstOption.choose(&empty).unwrap_err(),
            IllegalChoice::NoOptions { .. }
        ));
        assert!(matches!(
            SeededRandom::new(1).choose(&empty).unwrap_err(),
            IllegalChoice::NoOptions { .. }
        ));
    }

    #[test]
    fn a_script_states_the_exact_line_of_play() {
        let mut scripted = Scripted::new(["y", "decline"]);
        assert_eq!(scripted.choose(&three()).unwrap().id, "y");
        assert_eq!(scripted.choose(&three()).unwrap().id, "decline");
        assert_eq!(scripted.remaining(), 0);
    }

    #[test]
    fn a_script_that_diverges_fails_loudly() {
        // The point of a script: if the engine offers something unexpected the test must
        // fail, not quietly take a different line.
        let mut scripted = Scripted::new(["nonesuch"]);
        let err = scripted.choose(&three()).unwrap_err();
        assert!(
            matches!(err, IllegalChoice::ScriptDiverged { ref wanted, .. } if wanted == "nonesuch"),
            "{err}"
        );
    }

    #[test]
    fn an_exhausted_script_falls_back() {
        let mut scripted = Scripted::new(["y"]);
        assert_eq!(scripted.choose(&three()).unwrap().id, "y");
        assert_eq!(
            scripted.choose(&three()).unwrap().id,
            "x",
            "fallback is first"
        );
    }

    #[test]
    fn a_script_can_be_given_its_own_fallback() {
        let mut scripted = Scripted::with_fallback(Vec::<String>::new(), Box::new(AlwaysDecline));
        assert_eq!(scripted.choose(&three()).unwrap().id, "decline");
    }

    #[test]
    fn a_seeded_random_repeats_itself_and_only_picks_legal_options() {
        let run = || {
            let mut rng = SeededRandom::new(42);
            (0..20)
                .map(|_| rng.choose(&three()).unwrap().id)
                .collect::<Vec<_>>()
        };
        let first = run();
        assert_eq!(first, run(), "the same seed must play the same game");
        assert!(first.iter().all(|id| three().option(id).is_some()));
    }

    #[test]
    fn different_seeds_diverge() {
        let run = |seed| {
            let mut rng = SeededRandom::new(seed);
            (0..30)
                .map(|_| rng.choose(&three()).unwrap().id)
                .collect::<Vec<_>>()
        };
        assert_ne!(run(1), run(2));
    }

    // -- the table ----------------------------------------------------------------

    #[test]
    fn a_table_routes_a_choice_to_its_own_players_decider() {
        let mut table = Table::new();
        table.seat(pid("a"), Box::new(AlwaysDecline));
        assert_eq!(table.ask(&three()).unwrap().id, "decline");

        let other = Choice::new(pid("b"), "pick one", three().options);
        assert_eq!(table.ask(&other).unwrap().id, "x", "b uses the default");
    }

    #[test]
    fn a_table_records_every_answer() {
        let mut table = Table::new();
        table.ask(&three()).unwrap();
        table.ask(&three()).unwrap();

        assert_eq!(table.log.len(), 2);
        let record = &table.log.records[0];
        assert_eq!(record.player, pid("a"));
        assert_eq!(record.chosen, "x");
        assert_eq!(record.offered, vec!["x", "y", "decline"]);
        assert_eq!(record.prompt, "pick one");
    }

    #[test]
    fn a_table_rejects_an_invented_answer_before_recording_it() {
        struct Cheat;
        impl Decider for Cheat {
            fn choose(&mut self, _: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Ok(ChoiceOption::new("invented", "action"))
            }
        }
        let mut table = Table::new();
        table.seat(pid("a"), Box::new(Cheat));

        assert!(table.ask(&three()).is_err());
        assert!(table.log.is_empty(), "a rejected answer must not be logged");
    }

    #[test]
    fn an_identical_choice_asked_past_the_limit_makes_no_progress() {
        let mut table = Table::with_default(Box::new(AlwaysDecline));
        let same = three();
        for _ in 0..MAX_IDENTICAL_ASKS_PER_STEP {
            table.ask(&same).unwrap();
        }
        let logged = table.log.len();
        let err = table.ask(&same).expect_err("one ask too many");
        assert_eq!(
            err,
            IllegalChoice::NoProgress {
                player: pid("a"),
                prompt: "pick one".to_owned(),
                repeats: MAX_IDENTICAL_ASKS_PER_STEP + 1,
            }
        );
        let text = err.to_string();
        assert!(text.contains('a') && text.contains("pick one"), "{text}");
        assert_eq!(
            table.log.len(),
            logged,
            "the decider is not called or logged"
        );
        assert!(
            table.choice_error_since(0).is_some(),
            "recorded like other choice errors"
        );

        // A choice differing in one option id has its own count.
        let other = choice(vec![
            ChoiceOption::labelled("x", "action", "Do X"),
            ChoiceOption::labelled("z", "action", "Do Z"),
            ChoiceOption::decline(),
        ]);
        table.ask(&other).unwrap();

        // A new step starts the count afresh.
        table.begin_step();
        table.ask(&same).unwrap();
    }

    #[test]
    #[should_panic(expected = "ignoring the NoProgress refusal")]
    fn a_cycle_that_swallows_the_refusal_is_stopped() {
        let mut table = Table::with_default(Box::new(AlwaysDecline));
        let same = three();
        for _ in 0..=MAX_IDENTICAL_ASKS_PER_STEP * 10 {
            let _ = table.ask(&same);
        }
    }

    // -- the decision log -----------------------------------------------------------

    #[test]
    fn a_log_replays_as_a_script() {
        let mut table = Table::with_default(Box::new(AlwaysDecline));
        table.ask(&three()).unwrap();
        table.ask(&three()).unwrap();

        let script = table.log.as_script(None);
        assert_eq!(script, vec!["decline", "decline"]);

        // Feeding it back to a Scripted decider reproduces the same answers.
        let mut replay = Scripted::new(script);
        assert_eq!(replay.choose(&three()).unwrap().id, "decline");
        assert_eq!(replay.choose(&three()).unwrap().id, "decline");
    }

    #[test]
    fn a_log_can_be_filtered_to_one_player() {
        let mut table = Table::new();
        table.ask(&three()).unwrap();
        table
            .ask(&Choice::new(pid("b"), "pick one", three().options))
            .unwrap();

        assert_eq!(table.log.as_script(Some(&pid("a"))).len(), 1);
        assert_eq!(table.log.as_script(None).len(), 2);
    }

    #[test]
    fn a_seeded_run_replays_exactly_from_its_own_log() {
        // The determinism guarantee: a seed plus a decision log reproduces a game.
        let options = || {
            choice(vec![
                ChoiceOption::new("p", "action"),
                ChoiceOption::new("q", "action"),
                ChoiceOption::new("r", "action"),
            ])
        };
        let mut table = Table::with_default(Box::new(SeededRandom::new(7)));
        for _ in 0..25 {
            table.ask(&options()).unwrap();
        }

        let mut replay = Table::with_default(Box::new(Scripted::new(table.log.as_script(None))));
        for _ in 0..25 {
            replay.ask(&options()).unwrap();
        }
        assert_eq!(replay.log, table.log);
    }

    // -- distinguishable options -------------------------------------------------------

    #[test]
    fn interchangeable_units_are_offered_once() {
        // Five fighters are one option, not five. A sampling bot draws from the option
        // list, so writing a move five times gives it five times the probability of an
        // equally good move written once.
        let units = vec![
            unit("fighter", false),
            unit("fighter", false),
            unit("fighter", false),
            unit("dreadnought", false),
            unit("fighter", false),
        ];
        let distinct = distinct_units(&units);
        assert_eq!(distinct.len(), 2);
        assert_eq!(distinct[0].0, 0, "the first fighter");
        assert_eq!(distinct[1].0, 3, "the dreadnought");
    }

    #[test]
    fn a_damaged_unit_is_a_different_option_from_a_fresh_one() {
        // Losing a ship that has already taken a hit is a different proposition.
        let units = vec![
            unit("dreadnought", false),
            unit("dreadnought", true),
            unit("dreadnought", false),
        ];
        let distinct = distinct_units(&units);
        assert_eq!(distinct.len(), 2);
        assert!(!distinct[0].1.sustained_damage);
        assert!(distinct[1].1.sustained_damage);
    }

    #[test]
    fn the_kept_index_points_at_a_real_element() {
        let units = vec![unit("fighter", false), unit("carrier", false)];
        for (index, unit) in distinct_units(&units) {
            assert_eq!(&units[index], unit);
        }
    }

    #[test]
    fn first_of_each_keeps_the_original_order() {
        let items = ["b", "a", "b", "c", "a"];
        let firsts = first_of_each(&items, |s: &&str| *s);
        assert_eq!(
            firsts.iter().map(|(i, s)| (*i, **s)).collect::<Vec<_>>(),
            vec![(0, "b"), (1, "a"), (3, "c")]
        );
    }

    #[test]
    fn a_unit_label_shows_damage_rather_than_folding_it_away() {
        let dread = UnitTypeId::new("dreadnought");
        assert_eq!(unit_label("destroy", &dread, false), "destroy dreadnought");
        assert_eq!(
            unit_label("destroy", &dread, true),
            "destroy dreadnought (damaged)"
        );
    }

    #[test]
    fn options_are_built_from_id_label_pairs() {
        let built = options_from([("x", "Do X"), ("y", "Do Y")], "action");
        assert_eq!(built.len(), 2);
        assert_eq!(built[0].id, "x");
        assert_eq!(built[0].label, "Do X");
        assert_eq!(built[1].kind, "action");
    }

    #[test]
    fn a_choice_round_trips_through_json() {
        let json = serde_json::to_string(&three()).unwrap();
        assert_eq!(serde_json::from_str::<Choice>(&json).unwrap(), three());
    }

}
