//! The display facts of the action menu and the closing menu (`Choice.details`, package KM4, lane F): the string's
//! `Game::with_turn_menu`, computed from the `CoreState` alone. The web client reads them for the persistent action bar (the seat's token
//! pools, every strategy card with its used state and the option that plays it, every other seat with whether a transaction can be opened and,
//! when not, why, and what an action card would touch); no rule reads them. The oracle's choice comparison covers `details`, so a wrong
//! partner list or card state is caught.

use serde_json::{json, Map, Value};
use ti4_content::ContentStore;
use ti4_engine_core::choice::turn::{Ask as TurnAsk, Opt as TurnOpt};
use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Opt};
use ti4_engine_core::generated::{ActionCard, Planet};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::{board, trade};
use ti4_engine_core::state::CoreState;
use ti4_model::content_types::DEFAULT;

use crate::wire::choice::Choice;
use crate::generated::IdStr;

/// Complete the rendered action menu or closing menu of `choice`.
pub fn complete(core: &CoreState, choice: &CoreChoice, rendered: &mut Choice) {
    let closing = match choice.ask {
        Ask::Turn(TurnAsk::ActionMenu) => false,
        Ask::Turn(TurnAsk::Closing { .. }) => true,
        _ => return,
    };
    let Some(me) = core.players().by_seat(choice.actor) else { return };
    let offered_open = |other: Seat| choice.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TurnOpt::Open { with, .. }) if *with == other));
    let id_of = |seat: Seat| core.owners.id(seat).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();

    // Display only: what a card's play would touch, keyed by the option that plays it.
    let mut effects = Map::new();
    for (opt, shown) in choice.opts.as_slice().iter().zip(&rendered.options) {
        if let Opt::Turn(TurnOpt::Play { card: ActionCard::EconomicInitiative, .. }) = opt {
            effects.insert(shown.id.clone(), json!({ "kind": "ready_cultural", "planets": cultural_planets_preview(core, choice.actor) }));
        }
    }

    // Every strategy card the seat holds, in the order held. A lone unused card keeps the bare `strategic` id even when another card is
    // already spent, so the option is looked up by the card's name first and then by the bare id.
    let unused = ti4_engine_core::driver::menu_sources::unused_cards(core, choice.actor);
    let cards: Vec<Value> = me
        .strat
        .as_slice()
        .iter()
        .map(|card| {
            let named = format!("{}|{}", crate::ties::strategy::STRATEGIC_ACTION_ID, IdStr::as_str(*card));
            let option = rendered
                .options
                .iter()
                .find(|o| o.id == named)
                .or_else(|| (unused.as_slice() == [*card]).then(|| rendered.options.iter().find(|o| o.id == crate::ties::strategy::STRATEGIC_ACTION_ID)).flatten())
                .map(|o| o.id.clone());
            json!({ "card": IdStr::as_str(*card), "used": me.strat_exhausted.contains(*card), "option": option })
        })
        .collect();

    // Every other seat in the seating order: whether a transaction can be opened now and, when not, why.
    let kernel = crate::live::kernel::kernel_for(DEFAULT);
    let diplomacy = core.trade().diplomacy.enabled;
    let partners: Vec<Value> = core
        .seats()
        .seating_order()
        .iter()
        .copied()
        .filter(|other| *other != choice.actor)
        .filter_map(|other| {
            let theirs = core.players().by_seat(other)?;
            let neighbour = core.loaded.has(ti4_engine_core::state::Slice::Map)
                && ti4_engine_core::kernel::trade_env::with_cx(core, kernel, |cx| trade::may_transact(cx, choice.actor, other)).unwrap_or(false);
            let available = offered_open(other);
            let reason = if available {
                None
            } else if diplomacy {
                Some("Deals go through diplomatic contacts")
            } else if trade::transacted_with(core, choice.actor, other) {
                Some("Already traded with them this turn")
            } else if !neighbour {
                Some("No contact: not neighbours")
            } else {
                Some("Not available now")
            };
            let notes = core.cards().promissory.iter().filter(|(_, holder)| **holder == other).count();
            Some(json!({
                "seat": id_of(other),
                "faction": IdStr::as_str(theirs.faction),
                "available": available,
                "in_contact": neighbour,
                "reason": reason,
                "trade_goods": theirs.econ.trade_goods,
                "commodities": theirs.econ.commodities,
                "promissory_notes": notes,
            }))
        })
        .collect();

    let d = &mut rendered.details;
    d.insert("action_card_effects".to_owned(), Value::Object(effects));
    d.insert("kind".to_owned(), "turn_menu".into());
    d.insert("closing".to_owned(), closing.into());
    d.insert("actions_taken".to_owned(), core.pending().actions_this_turn.into());
    d.insert("tokens".to_owned(), json!({ "tactic": me.econ.tactic_tokens, "fleet": me.econ.fleet_tokens, "strategy": me.econ.strategic_tokens }));
    d.insert("strategy_cards".to_owned(), Value::Array(cards));
    d.insert("partners".to_owned(), Value::Array(partners));
}

/// `action_cards::cultural_planets_preview`: the cultural planets the seat controls and whether each is exhausted now.
fn cultural_planets_preview(core: &CoreState, seat: Seat) -> Vec<Value> {
    let content = ContentStore::embedded();
    board::controlled_planets(core, seat)
        .as_slice()
        .iter()
        .filter_map(|(_, planet): &(_, Planet)| {
            let id = IdStr::as_str(*planet);
            let record = ti4_content::galaxy::planet(content, id, DEFAULT)?;
            record.planet_type().is_some_and(|kind| kind.eq_ignore_ascii_case("cultural")).then(|| {
                json!({ "id": id, "name": record.name().unwrap_or(id), "trait": "cultural", "exhausted": board::exhausted(core, *planet) })
            })
        })
        .collect()
}
