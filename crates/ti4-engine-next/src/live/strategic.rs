//! The strategic action's share of the live loop (package KX, lane E): the display facts of the questions the follower window and the
//! card abilities ask that only the state and the content store know (`strategy::secondary_choice`, `strategy_cards::{purchase_details,
//! ready_hints, commodity_standing}`), computed from the core position and the kernel alone.

use serde_json::{json, Value};
use ti4_content::ContentStore;
use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::generated::StrategyCardKind;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::query::{board, reach};
use ti4_engine_core::rules::production::{self as prod, Cx as ProdCx, Held};
use ti4_engine_core::state::CoreState;
use ti4_model::content_types::SourceSet;

use crate::wire::choice::Choice;
use crate::generated::IdStr;

/// Influence one Leadership token costs (`strategy_cards::INFLUENCE_PER_TOKEN`).
const INFLUENCE_PER_TOKEN: i64 = 3;

/// The display facts of the strategic action's questions.
pub fn complete(core: &CoreState, kernel: &Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        Ask::Strategy(SAsk::Follow { card, .. }) => follow(core, kernel, choice.actor, card.data().kind, rendered),
        // Galactic Crisis Pact (`strategy::crisis_pact_choice`): the follower's question without `tokens_left`, with the agenda it comes from.
        Ask::Strategy(SAsk::CrisisPact { card }) => {
            follow(core, kernel, choice.actor, card.data().kind, rendered);
            rendered.details.remove("tokens_left");
            rendered.details.insert("agenda".to_owned(), crate::ties::strategy_cards::agenda_details(ContentStore::embedded(), "crisis"));
        }
        // The label of a technology is its printed name, which only the content store has.
        Ask::Strategy(SAsk::ResearchTechnology { .. }) => {
            let content = ContentStore::embedded();
            for (option, opt) in rendered.options.iter_mut().zip(choice.opts.iter()) {
                if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Research(t)) = opt {
                    option.label = crate::ties::technology::name(content, &ti4_model::id::TechnologyId::new(t.as_str()));
                }
            }
        }
        // Diplomacy: "ready which planet" shows where the planets and the cards that spend stand (`ready_planets`).
        Ask::Strategy(SAsk::ReadyPlanet) => {
            rendered.details.insert("ready_hints".to_owned(), ready_hints(core, kernel, choice.actor));
        }
        // Leadership's purchase (one question since the multi-ask conversion): the pools and what can be bought.
        Ask::Production(ti4_engine_core::choice::production::Ask::Purchase(_)) => {
            if let Some(purchase) = purchase_details(core, kernel, choice.actor) {
                super::details::pool_details(core, choice.actor, "buy", Some(0), rendered);
                rendered.details.insert("purchase".to_owned(), purchase);
            }
        }
        // Politics: the seat of each speaker option, the printed agenda (`strategy_cards_core::complete_politics`).
        Ask::Strategy(SAsk::PoliticsSpeaker) => {
            let mut seats = serde_json::Map::new();
            for (option, opt) in rendered.options.iter().zip(choice.opts.iter()) {
                if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Speaker(seat)) = opt {
                    seats.insert(option.id.clone(), Value::from(seat_text(core, *seat)));
                }
            }
            rendered.details.insert("seats".to_owned(), Value::Object(seats));
        }
        Ask::Strategy(SAsk::PoliticsAgenda { agenda }) => {
            rendered.details.insert("agenda".to_owned(), crate::ties::strategy_cards::agenda_details(ContentStore::embedded(), IdStr::as_str(agenda)));
        }
        // Both agendas at once (the first was the top card): the printed cards, in that order.
        Ask::Strategy(SAsk::PoliticsAgendas { first, second }) => {
            let cards = [first, second].map(|a| crate::ties::strategy_cards::agenda_details(ContentStore::embedded(), IdStr::as_str(a)));
            rendered.details.insert("agendas".to_owned(), Value::Array(cards.to_vec()));
        }
        // Trade's primary: the seat of each option and its commodities (`strategy_core::complete_trade`).
        Ask::Strategy(SAsk::TradeReplenish) => {
            let mut seats = serde_json::Map::new();
            let mut commodities = serde_json::Map::new();
            for (option, opt) in rendered.options.iter().zip(choice.opts.iter()) {
                if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(seat)) = opt {
                    seats.insert(option.id.clone(), Value::from(seat_text(core, *seat)));
                    let (have, max) = core.players().by_seat(*seat).map_or((0, 0), |p| (p.econ.commodities, p.faction.data().commodities));
                    commodities.insert(option.id.clone(), json!({ "have": have, "max": max }));
                }
            }
            rendered.details.insert("seats".to_owned(), Value::Object(seats));
            rendered.details.insert("commodities".to_owned(), Value::Object(commodities));
        }
        _ => {}
    }
}

/// The text of a seat (the player's id) in a position.
fn seat_text(core: &CoreState, seat: Seat) -> String {
    core.owners.id(seat).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default()
}

/// `strategy::secondary_choice`: the follower's secondary question with the facts a client needs to present it.
fn follow(core: &CoreState, kernel: &Kernel, who: Seat, kind: StrategyCardKind, rendered: &mut Choice) {
    let tokens_left = core.players().by_seat(who).map_or(0, |p| i64::from(p.econ.strategic_tokens));
    match kind {
        StrategyCardKind::Leadership => {
            // The window's question is the purchase question itself: it carries the pools and what a client needs to plan every purchase.
            super::details::pool_details(core, who, "buy", Some(0), rendered);
            if let Some(purchase) = purchase_details(core, kernel, who) {
                rendered.details.insert("purchase".to_owned(), purchase);
            }
        }
        StrategyCardKind::Diplomacy => {
            rendered.details.insert("ready_hints".to_owned(), ready_hints(core, kernel, who));
        }
        StrategyCardKind::Trade => {
            // Display only: the follower's commodities now and at the limit the replenish fills to.
            rendered.details.insert("commodities".to_owned(), commodity_standing(core, who));
        }
        _ => {}
    }
    // (`kind` is the area's marker of the question, not the pools': it is set last.)
    rendered.details.insert("kind".to_owned(), "strategy_secondary".into());
    rendered.details.insert("tokens_left".to_owned(), tokens_left.into());
    // A follower's question carries no context but Leadership's (the string asks it with `Choice::new`).
    if kind != StrategyCardKind::Leadership {
        rendered.context = None;
    }
}

/// `strategy_cards::commodity_standing`.
#[must_use]
pub fn commodity_standing(core: &CoreState, who: Seat) -> Value {
    let (have, goods, faction) = core
        .players()
        .by_seat(who)
        .map_or((0, 0, None), |p| (p.econ.commodities, p.econ.trade_goods, Some(p.faction)));
    json!({
        "have": have,
        "max": faction.map_or(0, ti4_engine_core::rules::strategy::commodity_limit),
        "trade_goods": goods,
    })
}

/// `strategy_cards::purchase_details`: what the Leadership purchase question shows. `None` below the price of one token.
#[must_use]
pub fn purchase_details(core: &CoreState, kernel: &Kernel, who: Seat) -> Option<Value> {
    let cx = ProdCx::new(core, kernel, reach::scope_of(kernel), reach::blanked(core));
    let held = Held::of(&cx, who);
    let ex = held.extras();
    let available = prod::available(&cx, who, Spend::Influence, &ex);
    if available < INFLUENCE_PER_TOKEN {
        return None;
    }
    let planets: Vec<Value> = prod::spendable_planets(&cx, who, &ex)
        .as_slice()
        .iter()
        .filter_map(|planet| {
            prod::payment_faces(&cx, who, *planet, Spend::Influence)
                .as_slice()
                .iter()
                .find(|(source, _)| *source == Spend::Influence)
                .map(|(_, worth)| json!({ "id": planet.as_str(), "worth": worth }))
        })
        .collect();
    let goods = core.players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods));
    Some(json!({
        "cost": INFLUENCE_PER_TOKEN,
        "influence_available": available,
        "max": available / INFLUENCE_PER_TOKEN,
        "trade_goods": goods,
        "trade_good_worth": prod::trade_good_worth(&cx, who),
        "planets": planets,
    }))
}

/// `strategy_cards::ready_hints`: the exhausted planets of the seat as they pay now, and where the cards that spend stand.
#[must_use]
pub fn ready_hints(core: &CoreState, kernel: &Kernel, who: Seat) -> Value {
    let content = ContentStore::embedded();
    // (A planet's printed name does not depend on the sources in play.)
    let sources = SourceSet::all();
    let cx = ProdCx::new(core, kernel, reach::scope_of(kernel), reach::blanked(core));
    let planets: Vec<Value> = board::controlled_planets(core, who)
        .as_slice()
        .iter()
        .map(|(_, planet)| *planet)
        .filter(|planet| board::exhausted(core, *planet))
        .map(|planet| {
            let record = ti4_content::galaxy::planet(content, planet.as_str(), sources);
            json!({
                "id": planet.as_str(),
                "name": record.as_ref().and_then(|r| r.name()).unwrap_or(planet.as_str()),
                // As the planet pays now (attachments and laws included), like the payment screens.
                "resources": record.as_ref().map(|_| prod::planet_value(&cx, who, planet, Spend::Resources)),
                "influence": record.as_ref().map(|_| prod::planet_value(&cx, who, planet, Spend::Influence)),
            })
        })
        .collect();
    let spenders: Vec<Value> = [(StrategyCardKind::Leadership, "influence"), (StrategyCardKind::Technology, "resources"), (StrategyCardKind::Warfare, "resources")]
        .iter()
        .map(|(kind, spends)| {
            let mut held = false;
            let mut played = false;
            for seat in core.players().iter() {
                for card in seat.strat.iter() {
                    if card.data().kind == *kind {
                        held = true;
                        played |= seat.strat_exhausted.contains(*card);
                    }
                }
            }
            let status = if !held {
                "not_in_play"
            } else if played {
                "played"
            } else {
                "to_come"
            };
            json!({ "card": printed_name(*kind), "spends": spends, "status": status })
        })
        .collect();
    json!({ "planets": planets, "spenders": spenders })
}

/// The printed name of a card kind (`StrategyCardKind::printed_name`).
const fn printed_name(kind: StrategyCardKind) -> &'static str {
    match kind {
        StrategyCardKind::Leadership => "Leadership",
        StrategyCardKind::Diplomacy => "Diplomacy",
        StrategyCardKind::Politics => "Politics",
        StrategyCardKind::Construction => "Construction",
        StrategyCardKind::Trade => "Trade",
        StrategyCardKind::Warfare => "Warfare",
        StrategyCardKind::Technology => "Technology",
        StrategyCardKind::Imperial => "Imperial",
    }
}
