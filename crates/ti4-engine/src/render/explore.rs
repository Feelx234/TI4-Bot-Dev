//! Renderer of the `explore` area: `impl Family` for `ti4_engine_core::choice::explore` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B5. The texts are those of `exploration.rs` (`choose_deck`, `explore_drawn`'s Distant
//! Suns, the `ion` card, `resolve_instant`'s reward questions), byte for byte; every option has kind `explore`.

use ti4_engine_core::choice::explore::{Ask, Opt, Step, Subtype};
use ti4_engine_core::generated::{Explore, ExploreType, Wormhole};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::core_sync::conv::explore_key;
use crate::generated::IdStr;

const KIND: &str = "explore";

/// The renderer of the exploration questions.
pub struct Fam;

/// The text of a step as the string engine names it (`DecisionSource::Content`, subtype prefix).
#[must_use]
pub const fn step_text(s: Step) -> &'static str {
    match s {
        Step::Exploration => "exploration",
        Step::IonStorm => "ion_storm",
        Step::AbandonedWarehouses => "abandoned_warehouses",
        Step::MerchantStation => "merchant_station",
        Step::FunctioningBase => "functioning_base",
        Step::LocalFabricators => "local_fabricators",
        Step::MercenaryOutfit => "mercenary_outfit",
    }
}

/// The step a text names.
#[must_use]
pub fn step_of(text: &str) -> Option<Step> {
    Step::ALL.into_iter().find(|s| step_text(*s) == text)
}

fn face_text(f: Wormhole) -> &'static str {
    if f == Wormhole::Beta { "BETA" } else { "ALPHA" }
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::ChooseDeck => "choose which deck to explore this planet into".to_owned(),
            Ask::DistantSuns { .. } => "Distant Suns: which exploration card to resolve".to_owned(),
            Ask::IonStorm => "Ion Storm: which side faceup".to_owned(),
            Ask::PayInfantry { .. } => "remove 1 infantry from this planet to resolve the exploration card".to_owned(),
            Ask::Reward(step) => match step {
                Step::AbandonedWarehouses => "Abandoned Warehouses",
                Step::MerchantStation => "Merchant Station",
                Step::FunctioningBase => "Functioning Base",
                Step::LocalFabricators => "Local Fabricators",
                _ => "Mercenary Outfit",
            }
            .to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::Step(step) => format!("{}_choose_reward", step_text(*step)),
            Subtype::Card(card) => format!("{}_choose_reward", card.as_str()),
        }
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let (id, label): (String, String) = match o {
            Opt::Deck(t) => (explore_key(*t), explore_key(*t)),
            Opt::Card(c) => (c.as_str().to_owned(), format!("resolve {}", c.as_str())),
            Opt::Face(f) => {
                (face_text(*f).to_owned(), if *f == Wormhole::Beta { "beta wormhole" } else { "alpha wormhole" }.to_owned())
            }
            Opt::Gain => (
                "gain".to_owned(),
                match a {
                    Ask::Reward(Step::AbandonedWarehouses) => "gain 2 commodities",
                    _ => "gain 1 commodity",
                }
                .to_owned(),
            ),
            Opt::Replenish => ("replenish".to_owned(), "replenish commodities".to_owned()),
            Opt::Convert => (
                "convert".to_owned(),
                match a {
                    Ask::Reward(Step::AbandonedWarehouses) => "convert up to 2 commodities to trade goods",
                    _ => "convert commodities to trade goods",
                }
                .to_owned(),
            ),
            Opt::SpendTradeGood => (
                "spend_tg".to_owned(),
                match a {
                    Ask::Reward(Step::FunctioningBase) => "spend 1 trade good to draw an action card",
                    _ => "spend 1 trade good to place a mech",
                }
                .to_owned(),
            ),
            Opt::SpendCommodity => (
                "spend_com".to_owned(),
                match a {
                    Ask::Reward(Step::FunctioningBase) => "spend 1 commodity to draw an action card",
                    _ => "spend 1 commodity to place a mech",
                }
                .to_owned(),
            ),
            Opt::Place => ("place".to_owned(), "place 1 infantry".to_owned()),
            Opt::PlaceNothing => ("decline".to_owned(), "place nothing".to_owned()),
            Opt::RemoveInfantry => ("remove_infantry".to_owned(), "remove 1 infantry and gain the reward".to_owned()),
            Opt::KeepInfantry => ("decline".to_owned(), "do not remove an infantry".to_owned()),
        };
        ChoiceOption::labelled(id, KIND, label)
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::ChooseDeck => ExploreType::ALL.into_iter().find(|t| explore_key(*t) == id).map(Opt::Deck),
            Ask::DistantSuns { first, second } => [*first, *second].into_iter().find(|c| c.as_str() == id).map(Opt::Card),
            Ask::IonStorm => match id {
                "ALPHA" => Some(Opt::Face(Wormhole::Alpha)),
                "BETA" => Some(Opt::Face(Wormhole::Beta)),
                _ => None,
            },
            Ask::PayInfantry { .. } => match id {
                "remove_infantry" => Some(Opt::RemoveInfantry),
                "decline" => Some(Opt::KeepInfantry),
                _ => None,
            },
            Ask::Reward(_) => match id {
                "gain" => Some(Opt::Gain),
                "replenish" => Some(Opt::Replenish),
                "convert" => Some(Opt::Convert),
                "spend_tg" => Some(Opt::SpendTradeGood),
                "spend_com" => Some(Opt::SpendCommodity),
                "place" => Some(Opt::Place),
                "decline" => Some(Opt::PlaceNothing),
                _ => None,
            },
        }
    }
}

/// An exploration card as the string ids name it (for tests).
#[must_use]
pub fn card_of(id: &str) -> Option<Explore> {
    Explore::from_id(id)
}
