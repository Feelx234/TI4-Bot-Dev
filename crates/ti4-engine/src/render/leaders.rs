//! Renderer of the `leaders` area: `impl Family` for `ti4_engine_core::choice::leaders` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Rear Admiral Farran offer (prompt
//! "Rear Admiral Farran: gain 1 trade good for the sustained damage", subtype `letnev_commander_trade_good`,
//! option `gain` of kind `letnev_commander_trade_good`, then the generic decline).

use ti4_engine_core::choice::leaders::{Ask, Opt, RoundTarget, Subtype};
use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::generated::{Planet, System, Technology, Unit};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The kind of the round agents' unit options (the string `leader_ssruu_round_agent_unit`).
pub const ROUND_KIND: &str = "leader_ssruu_round_agent_unit";

/// The id of a round agent's unit option (the string `borrowed_round_agents::target_id`).
#[must_use]
pub fn round_target_id(r: &Render, t: &RoundTarget) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        r.seat_text(t.owner),
        t.planet.map_or("space", |p| p.as_str()),
        t.unit.as_str(),
        t.index,
        u8::from(t.sustained) * 2 + u8::from(t.galvanized)
    )
}

fn round_target_of(r: &Render, id: &str) -> Option<RoundTarget> {
    let parts: Vec<&str> = id.split('|').collect();
    let [owner, place, unit, index, flags] = parts.as_slice() else { return None };
    let flags: u8 = flags.parse().ok().filter(|f| *f < 4)?;
    let t = RoundTarget {
        owner: r.seat(owner)?,
        planet: if *place == "space" { None } else { Some(Planet::from_id(place)?) },
        unit: Unit::from_id(unit)?,
        index: index.parse().ok()?,
        sustained: flags & 2 != 0,
        galvanized: flags & 1 != 0,
    };
    (round_target_id(r, &t) == id).then_some(t)
}

/// The display facts of a round agent's question (`Choice::offered`): the leader's card and, per option, where
/// the unit stands and whose it is. Needs the options, so the adapter calls it with the core question.
pub fn complete_round_agent(r: &Render, choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::Opt as COpt;
    let ti4_engine_core::choice::Ask::Leaders(Ask::RoundAgentUnit { agent }) = choice.ask else { return };
    let content = ti4_content::ContentStore::embedded();
    let captions: Vec<(String, serde_json::Value)> = choice
        .opts
        .iter()
        .filter_map(|o| {
            let COpt::Leaders(Opt::RoundAgentUnit(t)) = o else { return None };
            let place = t.planet.map_or_else(
                || "In the fleet".to_owned(),
                |planet| {
                    let name = ti4_content::galaxy::planet(content, planet.as_str(), ti4_model::content_types::DEFAULT)
                        .and_then(|record| record.name())
                        .unwrap_or(planet.as_str())
                        .to_owned();
                    format!("On {name}")
                },
            );
            Some((
                round_target_id(r, t),
                crate::choice::offer_caption_for_seat(
                    &format!("{} · {place}", t.unit.as_str()),
                    Some(&format!("Unit {} of that kind there", u32::from(t.index) + 1)),
                    r.seat_text(t.owner),
                ),
            ))
        })
        .collect();
    let captions: Vec<(&str, serde_json::Value)> = captions.iter().map(|(id, c)| (id.as_str(), c.clone())).collect();
    rendered.details = crate::choice::offer_details(crate::strategy_cards::leader_card(content, agent.as_str(), "agent"), Vec::new(), &captions);
}

pub const GAIN_ID: &str = "gain";
pub const KIND: &str = "letnev_commander_trade_good";
const INFANTRY_KIND: &str = "leader_xxchaagent_infantry";
const HACAN_KIND: &str = "leader_hacanagent_branch";
const JOLNAR_KIND: &str = "leader_jolnarhero_swap";

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
            Ask::L1z1xDestination => "Leader: which system to gather your ships in".to_owned(),
            Ask::JolnarSwap { .. } => "Leader: replace which technology with what".to_owned(),
            Ask::RoundAgentUnit { agent } => format!("{}: choose one unit for +1 combat die", agent.as_str()),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainCommander => KIND.to_owned(),
            Subtype::XxchaReadyPlanet => "leader_xxchaagent_ready_planet".to_owned(),
            Subtype::XxchaRemoveInfantry => "leader_xxchaagent_remove_infantry".to_owned(),
            Subtype::HacanBranch => HACAN_KIND.to_owned(),
            Subtype::L1z1xDestination => "leader_l1z1xhero_destination".to_owned(),
            Subtype::JolnarSwap => JOLNAR_KIND.to_owned(),
            Subtype::RoundAgentUnit => ROUND_KIND.to_owned(),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::GainTradeGood => ChoiceOption::labelled(GAIN_ID, KIND, "Gain 1 trade good"),
            Opt::Planet(p, system) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", id).with_planet(id, system.system.map(|s| s.as_str()))
            }
            Opt::System(system) => ChoiceOption::labelled(system.as_str(), "system", system.as_str()),
            Opt::Remove => ChoiceOption::labelled("remove", INFANTRY_KIND, "remove 1 infantry"),
            Opt::Keep => match a {
                Ask::JolnarSwap { held } => {
                    ChoiceOption::labelled(format!("keep|{}", held.as_str()), JOLNAR_KIND, format!("keep {}", held.as_str()))
                }
                _ => ChoiceOption::labelled("keep", INFANTRY_KIND, "keep it"),
            },
            Opt::Trade(t) => ChoiceOption::labelled(t.as_str(), JOLNAR_KIND, format!("trade for {}", t.as_str())),
            Opt::GainCommodities => ChoiceOption::labelled("self", HACAN_KIND, "gain 2 commodities"),
            Opt::Replenish(seat) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, HACAN_KIND, format!("replenish {id}"))
            }
            Opt::RoundAgentUnit(t) => ChoiceOption::labelled(
                round_target_id(r, t),
                ROUND_KIND,
                format!(
                    "{}'s {}{} (unit {})",
                    r.seat_text(t.owner),
                    t.unit.as_str(),
                    t.planet.map(|p| format!(" on {}", p.as_str())).unwrap_or_default(),
                    u32::from(t.index) + 1
                ),
            ),
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
            Ask::L1z1xDestination => System::from_id(id).map(Opt::System),
            Ask::JolnarSwap { held } => match id.strip_prefix("keep|") {
                Some(kept) => (kept == held.as_str()).then_some(Opt::Keep),
                None => Technology::from_id(id).map(Opt::Trade),
            },
            Ask::HacanBranch => {
                if id == "self" {
                    return Some(Opt::GainCommodities);
                }
                r.seat(id).map(Opt::Replenish)
            }
            Ask::RoundAgentUnit { .. } => round_target_of(r, id).map(Opt::RoundAgentUnit),
        }
    }
}
