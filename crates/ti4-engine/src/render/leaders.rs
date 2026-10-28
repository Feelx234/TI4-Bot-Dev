//! Renderer of the `leaders` area: `impl Family` for `ti4_engine_core::choice::leaders` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Rear Admiral Farran offer (prompt
//! "Rear Admiral Farran: gain 1 trade good for the sustained damage", subtype `letnev_commander_trade_good`,
//! option `gain` of kind `letnev_commander_trade_good`, then the generic decline).

use ti4_engine_core::choice::leaders::{Ask, Opt, Subtype};

use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::generated::Planet;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub const GAIN_ID: &str = "gain";
pub const KIND: &str = "letnev_commander_trade_good";
const INFANTRY_KIND: &str = "leader_xxchaagent_infantry";
const HACAN_KIND: &str = "leader_hacanagent_branch";

/// The renderer of the leader questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::SustainCommander => "Rear Admiral Farran: gain 1 trade good for the sustained damage".to_owned(),
            Ask::XxchaPlanet => "Leader: ready which planet".to_owned(),
            Ask::XxchaInfantry => "Leader: remove 1 infantry from that planet?".to_owned(),
            Ask::HacanBranch => "Leader: gain 2 commodities or replenish another player".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainCommander => KIND.to_owned(),
            Subtype::XxchaReadyPlanet => "leader_xxchaagent_ready_planet".to_owned(),
            Subtype::XxchaRemoveInfantry => "leader_xxchaagent_remove_infantry".to_owned(),
            Subtype::HacanBranch => HACAN_KIND.to_owned(),
        }
    }

    fn option(r: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::GainTradeGood => ChoiceOption::labelled(GAIN_ID, KIND, "Gain 1 trade good"),
            Opt::Planet(p, system) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", id).with_planet(id, system.system.map(|s| s.as_str()))
            }
            Opt::Remove => ChoiceOption::labelled("remove", INFANTRY_KIND, "remove 1 infantry"),
            Opt::Keep => ChoiceOption::labelled("keep", INFANTRY_KIND, "keep it"),
            Opt::GainCommodities => ChoiceOption::labelled("self", HACAN_KIND, "gain 2 commodities"),
            Opt::Replenish(seat) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, HACAN_KIND, format!("replenish {id}"))
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::SustainCommander => (id == GAIN_ID).then_some(Opt::GainTradeGood),
            Ask::XxchaPlanet => Planet::from_id(id).map(|p| Opt::Planet(p, Shown::default())),
            Ask::XxchaInfantry => match id {
                "remove" => Some(Opt::Remove),
                "keep" => Some(Opt::Keep),
                _ => None,
            },
            Ask::HacanBranch => {
                if id == "self" {
                    return Some(Opt::GainCommodities);
                }
                r.seat(id).map(Opt::Replenish)
            }
        }
    }
}
