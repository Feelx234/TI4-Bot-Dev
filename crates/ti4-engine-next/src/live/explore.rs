//! The exploration's share of the live loop (lane H4): the display facts of the questions the exploration windows ask that only the position
//! and the content store know. The string's twin is `exploring_dual::question_details` (the commodity standing of the four commodity cards and the
//! offer card of the infantry that pays), which the dual adapter applies to its own renders; the native path had neither.

use ti4_content::ContentStore;
use ti4_engine_core::choice::explore::{Ask as EAsk, PayReward, Step};
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::state::CoreState;
use ti4_model::id::PlanetId;

use crate::wire::choice::Choice;
use crate::generated::IdStr;

/// The display facts of the exploration's questions.
pub fn complete(core: &CoreState, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        Ask::Explore(EAsk::Reward(Step::AbandonedWarehouses | Step::MerchantStation | Step::FunctioningBase | Step::LocalFabricators)) => {
            rendered.details.insert("commodities".to_owned(), super::strategic::commodity_standing(core, choice.actor));
        }
        Ask::Explore(EAsk::PayInfantry { card, planet, reward }) => {
            let system = ti4_engine_core::query::board::home_system(core, planet).map(|s| IdStr::as_str(s).to_owned()).unwrap_or_default();
            let reward = match reward {
                PayReward::TradeGood => "gain 1 trade good",
                PayReward::ReadyPlanet => "ready this planet",
                PayReward::CommandToken => "gain 1 command token",
            };
            let planet_id = PlanetId::new(IdStr::as_str(planet));
            let offer = crate::ties::exploration::pay_offer(ContentStore::embedded(), IdStr::as_str(card), &planet_id, &system, reward);
            *rendered = offer(rendered.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Ctx, Opts, Origin, Subtype};
    use ti4_engine_core::containers::SmallList;
    use ti4_engine_core::generated::{Explore, Faction, Planet};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::state::{CoreState, Player, Players};

    use super::*;

    fn asked(ask: EAsk) -> CoreChoice {
        CoreChoice {
            actor: Seat(0),
            ask: Ask::Explore(ask),
            ctx: Ctx { origin: Origin::Content(Content::Explore(Explore::AbandonedWarehouses1)), subtype: Subtype::Generic(ti4_engine_core::choice::generic::Subtype::Unclassified), optional: false, target: None, outstanding: SmallList::new(), space_battle: false, invasion_seq: None },
            opts: Opts::default(),
        }
    }

    fn blank() -> Choice {
        Choice { player: ti4_model::id::PlayerId::new("a"), prompt: String::new(), options: Vec::new(), context: None, details: serde_json::Map::new(), free_form: None }
    }

    #[test]
    fn the_commodity_cards_show_the_standing_and_the_infantry_that_pays_shows_the_offer_as_the_string_decorates_it() {
        let mut core = CoreState::all_loaded();
        let mut p = Player::new(Seat(0), Faction::TheFederationOfSol);
        p.econ.commodities = 2;
        p.econ.trade_goods = 5;
        core.players = Players::from_vec(vec![p]);
        let mut shown = blank();
        complete(&core, &asked(EAsk::Reward(Step::AbandonedWarehouses)), &mut shown);
        assert_eq!(shown.details.get("commodities"), Some(&serde_json::json!({ "have": 2, "max": 4, "trade_goods": 5 })));
        let mut none = blank();
        complete(&core, &asked(EAsk::IonStorm), &mut none);
        assert!(none.details.is_empty(), "no other question of the area is decorated");
        let planet = Planet::ALL.into_iter().find(|p| p.data().system.is_some()).expect("a printed planet");
        let mut offer = blank();
        complete(&core, &asked(EAsk::PayInfantry { card: Explore::Expedition1, planet, reward: PayReward::ReadyPlanet }), &mut offer);
        assert_eq!(offer.details.get("kind"), Some(&serde_json::json!("offer")));
        assert_eq!(offer.details["card"]["title"], "Expedition");
        assert_eq!(offer.details["facts"][1], serde_json::json!({ "label": "You lose", "value": "1 infantry on this planet" }));
        assert_eq!(offer.details["facts"][2], serde_json::json!({ "label": "You gain", "value": "ready this planet" }));
        assert_eq!(offer.details["facts"][0]["system"], IdStr::as_str(planet.data().system.unwrap()));
    }
}
