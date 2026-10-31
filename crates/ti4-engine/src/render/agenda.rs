//! Renderer of the `agenda` area: `impl Family` for `ti4_engine_core::choice::agenda` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B9 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the seven questions of `vote::VoteWindow`, byte
//! for byte (prompts, option ids, kinds, labels, payload, `details`). The texts below are the string
//! engine's; the renderer sees no state, so every number an option shows is carried by the ask or the option.

use serde_json::{Map, Value, json};
use ti4_content::ContentStore;
use ti4_engine_core::choice::agenda::{Ask, Face, Frame, Opt, Shown, Subtype, UseSource};
use ti4_engine_core::generated::{Agenda, AgendaElects, Planet, SecretObjective, StrategyCard};
use ti4_engine_core::keys::Outcome;
use ti4_model::content_types::ContentType;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;
use crate::preview::{Delta, Preview, Quantity};

pub const VOTE_KIND: &str = "vote";
pub const VOTE_PLANET_KIND: &str = "vote_planet";
pub const TIEBREAK_KIND: &str = "tiebreak";
pub const VOTE_TRADE_GOODS_KIND: &str = "vote_trade_goods";
pub const PREDICTIVE_KIND: &str = "vote_predictive";
pub const RECOMBINE_KIND: &str = "recombine";
pub const TRIBUTE_KIND: &str = "tribute";
const RESOURCES_SUFFIX: &str = "|resources";
/// `vote::PREDICTIVE_VOTES`.
pub const PREDICTIVE_VOTES: i64 = 3;

/// The renderer of the agenda questions.
pub struct Fam;

/// The text of an outcome (the owner id for a seat).
#[must_use]
pub fn outcome_text(r: &Render, o: Outcome) -> String {
    match o {
        Outcome::Empty => String::new(),
        Outcome::For => "for".to_owned(),
        Outcome::Against => "against".to_owned(),
        Outcome::Seat(s) => r.seat_text(s).to_owned(),
        Outcome::Planet(p) => p.as_str().to_owned(),
        Outcome::StrategyCard(c) => c.as_str().to_owned(),
        Outcome::Law(a) => a.as_str().to_owned(),
        Outcome::Secret(s) => s.as_str().to_owned(),
    }
}

/// `vote::outcome_label`: a strategy card or a secret objective by its printed name, everything else as
/// the outcome itself.
pub(super) fn outcome_label(r: &Render, frame: &Frame, o: Outcome) -> String {
    let text = outcome_text(r, o);
    let category = match frame.voted().data().elects {
        AgendaElects::StrategyCard => ContentType::StrategyCards,
        AgendaElects::ScoredSecretObjective => ContentType::SecretObjectives,
        _ => return text,
    };
    ContentStore::embedded()
        .get(category, &text)
        .and_then(|record| record.text("name"))
        .map_or(text, ToOwned::to_owned)
}

fn planet_outcomes(frame: &Frame) -> bool {
    matches!(frame.voted().data().elects, AgendaElects::Planet | AgendaElects::NonHomeNonMecatolPlanet)
}

fn votes_delta(from: i64, to: i64) -> Preview {
    Preview::certain(vec![Delta::new(Quantity::Votes, from, to)])
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::Vote { .. } => "vote for which outcome".to_owned(),
            Ask::Planets { outcome, .. } => format!("exhaust a planet to vote {}", outcome_text(r, *outcome)),
            Ask::TradeGoods { outcome, .. } => format!("spend trade goods for votes on {}", outcome_text(r, *outcome)),
            Ask::Predictive { outcome, .. } => {
                format!("Predictive Intelligence: 3 more votes for {}", outcome_text(r, *outcome))
            }
            Ask::Recombine { voter, .. } => format!("Genetic Recombination before {} votes", r.seat_text(*voter)),
            Ask::Tribute { outcome, .. } => {
                format!("Genetic Recombination: vote {} or return a fleet token", outcome_text(r, *outcome))
            }
            Ask::Tiebreak { .. } => "speaker breaks the tie".to_owned(),
            Ask::NameTied => "which tied player does the agenda name".to_owned(),
            Ask::Settler => "which of the trailing players may settle the planet".to_owned(),
            Ask::Structure => "destroy one of your structures".to_owned(),
            Ask::DiscardLaw { prompt, .. } => r.code_text(*prompt),
            Ask::Committee { agenda } => format!("{}: elect which player", agenda.as_str()),
            Ask::UseOnAgenda { source: UseSource::Quash, .. } => {
                "Quash: spend 1 strategy token to discard this agenda and reveal the next".to_owned()
            }
            Ask::UseOnAgenda { source: UseSource::Favor, .. } => "Political Favor: discard this agenda and reveal the next".to_owned(),
            Ask::UseOnAgenda { source: UseSource::Secret, owner, .. } => {
                format!("Political Secret: {} cannot vote on this agenda", r.seat_text(*owner))
            }
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::CastVote => "cast_vote",
            Subtype::ExhaustPlanet => "vote_exhaust_planet",
            Subtype::SpendTradeGoods => "vote_spend_trade_goods",
            Subtype::PredictiveIntelligence => "vote_predictive_intelligence",
            Subtype::RecombinationOutcome => "recombination_outcome",
            Subtype::RecombinationTribute => "recombination_tribute",
            Subtype::Tiebreak => "vote_tiebreak",
            Subtype::ElectTiebreak => "agenda_elect_tiebreak",
            Subtype::SettleTrailing => "redistribution_choose_settler",
            Subtype::DefenseStructure => "defense_act_choose_pds",
            Subtype::OfferDiscardLaw => "offer_discard_law",
            Subtype::UseQuash => "quash",
            Subtype::UseFavor => "favor",
            Subtype::UseSecret => "ps",
            Subtype::CommitteeElect => "committee_formation_elect",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::Committee { .. }, Opt::Elect(seat)) => ChoiceOption::labelled(r.seat_text(*seat), "player", format!("elect {}", r.seat_text(*seat))),
            (Ask::NameTied | Ask::Settler, Opt::Elect(seat)) => ChoiceOption::labelled(r.seat_text(*seat), "elect", r.seat_text(*seat)),
            (Ask::Structure, Opt::Structure { index, planet, system }) => {
                ChoiceOption::labelled(index.to_string(), "scuttle", format!("destroy the one on {}", planet.as_str()))
                    .with_planet(planet.as_str(), Some(system.as_str()))
            }
            (Ask::DiscardLaw { law, .. }, Opt::DiscardLaw) => ChoiceOption::labelled("discard", "law", format!("discard {}", law.as_str())),
            (Ask::UseOnAgenda { source, .. }, Opt::UseCard) => {
                let alias = match source {
                    UseSource::Quash => "quash",
                    UseSource::Favor => "favor",
                    UseSource::Secret => "ps",
                };
                ChoiceOption::labelled("use", "promissory_note", format!("use {alias}"))
            }
            _ => Self::vote_option(r, a, o),
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::NameTied | Ask::Settler | Ask::Committee { .. } => r.seat(id).map(Opt::Elect),
            // The planet and the system are the position's: the pick is matched against the offered options.
            Ask::Structure => None,
            Ask::DiscardLaw { .. } => (id == "discard").then_some(Opt::DiscardLaw),
            Ask::UseOnAgenda { .. } => (id == "use").then_some(Opt::UseCard),
            _ => Self::parse_vote(r, a, id),
        }
    }

    fn details(r: &Render, a: &Ask) -> Map<String, Value> {
        if let Ask::UseOnAgenda { agenda, .. } = a {
            // The card the question is about rides along as on every vote decision.
            let mut out = Map::new();
            out.insert("agenda".to_owned(), crate::strategy_cards::agenda_details(ContentStore::embedded(), agenda.as_str()));
            return out;
        }
        if a.frame().is_none() {
            return Map::new();
        }
        Self::vote_details(r, a)
    }
}

impl Fam {
    fn vote_option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let frame = a.frame().expect("a vote question");
        match (a, o) {
            (Ask::Vote { .. } | Ask::Tiebreak { .. }, Opt::Back(outcome, shown)) => {
                let id = outcome_text(r, *outcome);
                let vote = matches!(a, Ask::Vote { .. });
                let mut option =
                    ChoiceOption::labelled(&id, if vote { VOTE_KIND } else { TIEBREAK_KIND }, outcome_label(r, frame, *outcome));
                if vote {
                    option = option.with("current_votes", i64::from(shown.votes));
                }
                if planet_outcomes(frame) && !matches!(outcome, Outcome::For | Outcome::Against) {
                    option = option.with_planet(&id, shown.system.map(|s| s.as_str()));
                }
                option
            }
            (Ask::Planets { votes, spender, .. }, Opt::Exhaust { planet, face, shown }) => {
                let planet_id = planet.as_str();
                let cast = i64::from(shown.votes);
                let id = match face {
                    Face::Influence => planet_id.to_owned(),
                    Face::Resources => format!("{planet_id}{RESOURCES_SUFFIX}"),
                };
                let label = if *spender {
                    let face = match face {
                        Face::Resources => "resources",
                        Face::Influence => "influence",
                    };
                    format!("exhaust {planet_id} for {cast} votes ({face})")
                } else {
                    format!("exhaust {planet_id} for {cast} votes")
                };
                let so_far = i64::from(*votes);
                ChoiceOption::labelled(id, VOTE_PLANET_KIND, label)
                    .with_planet(planet_id, shown.system.map(|s| s.as_str()))
                    .previewed(votes_delta(so_far, so_far + cast))
            }
            (Ask::TradeGoods { votes, rate, .. }, Opt::Spend(n)) => {
                let (spent, rate) = (i64::from(*n), i64::from(*rate));
                ChoiceOption::labelled(
                    format!("spend|{n}"),
                    VOTE_TRADE_GOODS_KIND,
                    format!("spend {n} trade goods for {} votes", rate * spent),
                )
                .with("trade_goods", spent)
                .previewed(votes_delta(i64::from(*votes), i64::from(*votes) + rate * spent))
            }
            (Ask::Predictive { outcome, votes, .. }, Opt::CastPredictive) => ChoiceOption::labelled(
                "predictive|cast",
                PREDICTIVE_KIND,
                format!("cast 3 additional votes for {}", outcome_text(r, *outcome)),
            )
            .with("votes", PREDICTIVE_VOTES)
            .previewed(votes_delta(i64::from(*votes), i64::from(*votes) + PREDICTIVE_VOTES)),
            (Ask::Recombine { voter, .. }, Opt::Demand(outcome)) => {
                let text = outcome_text(r, *outcome);
                ChoiceOption::labelled(
                    format!("recombine|{text}"),
                    RECOMBINE_KIND,
                    format!("{} must vote {text} or return a fleet token", r.seat_text(*voter)),
                )
            }
            (Ask::Tribute { outcome, .. }, Opt::TributeVote) => ChoiceOption::labelled(
                "tribute|vote",
                TRIBUTE_KIND,
                format!("cast at least 1 vote for {}", outcome_text(r, *outcome)),
            ),
            (Ask::Tribute { .. }, Opt::TributeToken) => ChoiceOption::labelled(
                "tribute|token",
                TRIBUTE_KIND,
                "remove 1 token from your fleet pool and return it to reinforcements",
            ),
            (a, o) => panic!("option {o:?} in a question {a:?}"),
        }
    }

    fn parse_vote(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        let frame = a.frame()?;
        match a {
            Ask::Vote { .. } | Ask::Tiebreak { .. } => {
                parse_outcome(r, frame, id).map(|o| Opt::Back(o, Shown::default()))
            }
            Ask::Planets { .. } => {
                let (planet, face) = match id.strip_suffix(RESOURCES_SUFFIX) {
                    Some(p) => (p, Face::Resources),
                    None => (id, Face::Influence),
                };
                Some(Opt::Exhaust { planet: Planet::from_id(planet)?, face, shown: Shown::default() })
            }
            Ask::TradeGoods { .. } => {
                let n: u8 = id.strip_prefix("spend|")?.parse().ok()?;
                (n.to_string() == id["spend|".len()..]).then_some(Opt::Spend(n))
            }
            Ask::Predictive { .. } => (id == "predictive|cast").then_some(Opt::CastPredictive),
            Ask::Recombine { .. } => {
                let outcome = parse_outcome(r, frame, id.strip_prefix("recombine|")?)?;
                Some(Opt::Demand(outcome))
            }
            Ask::Tribute { .. } => match id {
                "tribute|vote" => Some(Opt::TributeVote),
                "tribute|token" => Some(Opt::TributeToken),
                _ => None,
            },
            Ask::NameTied | Ask::Settler | Ask::Structure | Ask::DiscardLaw { .. } | Ask::UseOnAgenda { .. } | Ask::Committee { .. } => None,
        }
    }

    fn vote_details(r: &Render, a: &Ask) -> Map<String, Value> {
        let frame = a.frame().expect("a vote question");
        let mut out = Map::new();
        let content = ContentStore::embedded();
        out.insert("agenda".to_owned(), crate::strategy_cards::agenda_details(content, frame.agenda.as_str()));
        if frame.reveal
            && let Some(hidden) = frame.covert
            && frame.agenda == Agenda::CovertLegislation
        {
            out.insert("covert_agenda".to_owned(), crate::strategy_cards::agenda_details(content, hidden.as_str()));
        }
        match a {
            Ask::Planets { outcome: _, votes, tally, .. } => {
                let entries: Vec<Value> =
                    tally.as_slice().iter().map(|(o, n)| json!([outcome_text(r, *o), i64::from(*n)])).collect();
                out.insert("tally".to_owned(), Value::Array(entries));
                out.insert("votes_so_far".to_owned(), json!(i64::from(*votes)));
            }
            Ask::TradeGoods { outcome, votes, goods, .. } => {
                out.insert("kind".to_owned(), json!("vote_trade_goods"));
                out.insert("card".to_owned(), crate::strategy_cards::commander_card(content, "hacancommander"));
                out.insert("outcome".to_owned(), json!(outcome_text(r, *outcome)));
                out.insert("votes".to_owned(), json!(i64::from(*votes)));
                out.insert("goods".to_owned(), json!(i64::from(*goods)));
                out.insert("votes_per_good".to_owned(), json!(2));
            }
            Ask::Predictive { outcome, votes, .. } => {
                out.insert("outcome".to_owned(), json!(outcome_text(r, *outcome)));
                out.insert("votes".to_owned(), json!(i64::from(*votes)));
                out.insert("votes_added".to_owned(), json!(PREDICTIVE_VOTES));
            }
            _ => {}
        }
        out
    }
}

/// The outcome an id names in a vote on `frame`'s voted agenda.
pub(super) fn parse_outcome(r: &Render, frame: &Frame, id: &str) -> Option<Outcome> {
    match id {
        "for" => return Some(Outcome::For),
        "against" => return Some(Outcome::Against),
        _ => {}
    }
    match frame.voted().data().elects {
        AgendaElects::Player => r.seat(id).map(Outcome::Seat),
        AgendaElects::Planet | AgendaElects::NonHomeNonMecatolPlanet => Planet::from_id(id).map(Outcome::Planet),
        AgendaElects::StrategyCard => StrategyCard::from_id(id).map(Outcome::StrategyCard),
        AgendaElects::Law => Agenda::from_id(id).map(Outcome::Law),
        AgendaElects::ScoredSecretObjective => SecretObjective::from_id(id).map(Outcome::Secret),
        AgendaElects::ForAgainst | AgendaElects::HiddenNextAgenda => None,
    }
}
