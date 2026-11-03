//! Renderer of the `leaders` area: `impl Family` for `ti4_engine_core::choice::leaders` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Rear Admiral Farran offer (prompt
//! "Rear Admiral Farran: gain 1 trade good for the sustained damage", subtype `letnev_commander_trade_good`,
//! option `gain` of kind `letnev_commander_trade_good`, then the generic decline). Xxekir Grom (Thunder's Edge Xxcha
//! hero): `leader_xxchahero_te_place` / `_stop` / `_planet`.

use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::choice::leaders::{Ask, HeroUnit, Opt, RoundTarget, Subtype};
use ti4_engine_core::generated::{Planet, System, Technology, Unit, UnitBaseType};

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
    let ti4_engine_core::choice::Ask::Leaders(Ask::RoundAgentUnit { agent, copied }) = choice.ask else { return };
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
    rendered.details = crate::choice::offer_details(crate::strategy_cards::leader_card(content, agent.as_str(), if copied { "agent (copied by Ssruu)" } else { "agent" }), Vec::new(), &captions);
}

pub const GAIN_ID: &str = "gain";
pub const KIND: &str = "letnev_commander_trade_good";
const INFANTRY_KIND: &str = "leader_xxchaagent_infantry";
const HACAN_KIND: &str = "leader_hacanagent_branch";
const JOLNAR_KIND: &str = "leader_jolnarhero_swap";
const HERO_PLACE_KIND: &str = "leader_xxchahero_te_place";
const HERO_STOP_KIND: &str = "leader_xxchahero_te_stop";
const HERO_PLANET_KIND: &str = "leader_xxchahero_te_planet";

/// The name of a command token pool in Nekro Malleon's ids (`token|strategy`).
const fn pool_name(pool: Pool) -> &'static str {
    match pool {
        Pool::Tactic => "tactic",
        Pool::Fleet => "fleet",
        Pool::Strategic => "strategy",
    }
}

/// The printed name of a technology (`technology::name`: the corpus record's name, else the alias).
fn technology_name(tech: Technology) -> String {
    let content = ti4_content::ContentStore::embedded();
    content
        .get(ti4_model::content_types::ContentType::Technologies, tech.as_str())
        .and_then(|record| record.text("name"))
        .unwrap_or_else(|| tech.as_str())
        .to_owned()
}

/// The printed name of an action card (`action_cards::name_of`).
fn action_card_name(card: ti4_engine_core::generated::ActionCard) -> String {
    let content = ti4_content::ContentStore::embedded();
    content
        .get(ti4_model::content_types::ContentType::ActionCards, card.as_str())
        .and_then(|record| record.text("name"))
        .unwrap_or_else(|| card.as_str())
        .to_owned()
}

/// The renderer of the leader questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::OsphaShip => "Letani Ospha: choose a non-fighter ship".to_owned(),
            Ask::OsphaReplace { owner, ship, system } => format!("Letani Ospha: replace {}'s {} in {}", r.seat_text(*owner), ship.as_str(), system.as_str()),
            Ask::SustainCommander => "Rear Admiral Farran: gain 1 trade good for the sustained damage".to_owned(),
            Ask::XxchaPlanet => "Leader: ready which planet".to_owned(),
            Ask::XxchaInfantry => "Leader: remove 1 infantry from that planet?".to_owned(),
            Ask::HacanBranch => "Leader: gain 2 commodities or replenish another player".to_owned(),
            Ask::L1z1xDestination => "Leader: which system to gather your ships in".to_owned(),
            Ask::JolnarSwap { .. } => "Leader: replace which technology with what".to_owned(),
            Ask::RoundAgentUnit { agent, copied: false } => format!("{}: choose one unit for +1 combat die", agent.as_str()),
            Ask::RoundAgentUnit { agent, copied: true } => format!("Ssruu copying {}: choose one unit for +1 combat die", agent.as_str()),
            Ask::XxchaHeroPlace => "Leader: place up to 4 PDS or mechs (how many more?)".to_owned(),
            Ask::XxchaHeroPlanet => "Leader: which planet gets the unit?".to_owned(),
            Ask::SaarHero => "Gurno Aggero: destroy all other players' infantry and fighters in which system".to_owned(),
            Ask::GhostSwapFirst => "Riftwalker Meian: swap the positions of which system".to_owned(),
            Ask::GhostSwapSecond => "Riftwalker Meian: swap it with which system".to_owned(),
            Ask::NekroPlayer => "Nekro Malleon: choose a player".to_owned(),
            Ask::NekroPayment => "Nekro Malleon: discard 1 action card or spend 1 command token to gain 2 trade goods?".to_owned(),
            Ask::UmbatPlayer => "Umbat: choose a player who may produce up to 2 units costing 4 or less".to_owned(),
            Ask::UmbatSystem => "Umbat: produce in which system".to_owned(),
            Ask::FlayeshPlanet => "UNIT.DSGN.FLAYESH: choose a planet with a technology specialty".to_owned(),
            Ask::FlayeshTech => "UNIT.DSGN.FLAYESH: choose a technology that matches the specialty".to_owned(),
            Ask::KyverFate { target, card } => format!("Kyver: {} showed {}", r.seat_text(*target), action_card_name(*card)),
            Ask::NaazCard => "Hesh and Prit: perform the secondary of which card".to_owned(),
            Ask::WinnuCard => "Mathis Mathinus: which strategy card".to_owned(),
            Ask::WinnuFollower { follower } => format!("Mathis Mathinus: may {} follow the secondary", r.seat_text(*follower)),
            Ask::FlockShip => "Flock Migration: choose a ship to move".to_owned(),
            Ask::FlockTo => "Flock Migration: choose the destination".to_owned(),
            Ask::FlockCargo => "Flock Migration: carry a unit aboard (it cannot be landed)".to_owned(),
            Ask::MahactMove => "Airo Shir Aur: which fleet moves where".to_owned(),
            Ask::YinCommit { n, stock } => format!("Quantum Dissemination: commit infantry {} of {stock}", u32::from(*n) + 1),
            Ask::ZeuSystem => "Z'eu: perform a tactical action without placing a command token".to_owned(),
            Ask::SardakkSkip => "Sh'val: skip directly to the Commit Ground Forces step".to_owned(),
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
            Subtype::XxchaHeroPlace => HERO_PLACE_KIND.to_owned(),
            Subtype::XxchaHeroPlanet => HERO_PLANET_KIND.to_owned(),
            Subtype::SaarHero => "armageddon_relay".to_owned(),
            Subtype::GhostSwapFirst => "swap_first".to_owned(),
            Subtype::GhostSwapSecond => "swap_second".to_owned(),
            Subtype::OsphaShip => "ospha_ship".to_owned(),
            Subtype::OsphaReplace => "ospha_replace".to_owned(),
            Subtype::NekroPlayer => "malleon_player".to_owned(),
            Subtype::NekroPayment => "malleon_payment".to_owned(),
            Subtype::UmbatPlayer => "umbat_player".to_owned(),
            Subtype::UmbatSystem => "umbat_system".to_owned(),
            Subtype::FlayeshPlanet => "flayesh_planet".to_owned(),
            Subtype::FlayeshTech => "flayesh_technology".to_owned(),
            Subtype::KyverFate => "kyver_take_or_discard".to_owned(),
            Subtype::HeroCard => "hero_card".to_owned(),
            Subtype::HeroFollower => "hero_follower".to_owned(),
            Subtype::FlockShip => "flock_migration_ship".to_owned(),
            Subtype::FlockTo => "flock_migration_to".to_owned(),
            Subtype::FlockCargo => "flock_migration_cargo".to_owned(),
            Subtype::MahactMove => "mahact_hero_move".to_owned(),
            Subtype::YinCommit => "yin_hero_commit".to_owned(),
            Subtype::ZeuSystem => "zeu_activate_system".to_owned(),
            Subtype::SardakkSkip => "sardakk_hero_skip".to_owned(),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::GainTradeGood => ChoiceOption::labelled(GAIN_ID, KIND, "Gain 1 trade good"),
            Opt::SardakkSkip => ChoiceOption::labelled("skip", "leader", "skip to commit"),
            Opt::Planet(p, system) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", id).with_planet(id, system.system.map(|s| s.as_str()))
            }
            Opt::System(system) if matches!(a, Ask::ZeuSystem) => {
                ChoiceOption::labelled(system.as_str(), crate::tactical::ACTIVATE_KIND, format!("activate {} without a command token", system.as_str()))
            }
            Opt::System(system) if matches!(a, Ask::SaarHero | Ask::GhostSwapFirst | Ask::GhostSwapSecond | Ask::UmbatSystem) => ChoiceOption::labelled(system.as_str(), "system", format!("system {}", system.as_str())),
            Opt::System(system) => ChoiceOption::labelled(system.as_str(), "system", system.as_str()),
            Opt::OsphaShip { n, owner, ship, damaged, system } => ChoiceOption::labelled(
                format!("ship|{n}"),
                "ship",
                format!("{}'s {}{} in {}", r.seat_text(*owner), ship.as_str(), if *damaged { " (damaged)" } else { "" }, system.as_str()),
            ),
            Opt::OsphaReplace(base) => {
                let base = super::promissory::base_name_of(*base);
                ChoiceOption::labelled(format!("replace|{base}"), "ship", format!("replace it with a {base} from reinforcements"))
            }
            Opt::Player(seat) if matches!(a, Ask::UmbatPlayer) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, "player", format!("player {id}"))
            }
            Opt::Player(seat) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, "player", id)
            }
            Opt::Discard => ChoiceOption::labelled("discard", "nekro_malleon", "discard 1 action card to gain 2 trade goods"),
            Opt::Token(pool) => {
                let name = pool_name(*pool);
                ChoiceOption::labelled(format!("token|{name}"), "nekro_malleon", format!("spend 1 {name} command token to gain 2 trade goods"))
            }
            Opt::Devour(system, planet) => ChoiceOption::labelled(
                format!("{}|{}", system.as_str(), planet.as_str()),
                "nekro_flayesh_planet",
                format!("devour {} in {}", planet.as_str(), system.as_str()),
            ),
            Opt::Gain(tech) => ChoiceOption::labelled(tech.as_str(), "nekro_flayesh_technology", technology_name(*tech)),
            Opt::KyverTake => {
                let Ask::KyverFate { target, card } = a else { unreachable!("a Kyver option under another question") };
                ChoiceOption::labelled("take", "hero", format!("take {} from {}", action_card_name(*card), r.seat_text(*target)))
            }
            Opt::KyverDiscard => {
                let Ask::KyverFate { target, .. } = a else { unreachable!("a Kyver option under another question") };
                ChoiceOption::labelled("discard", "hero", format!("force {} to discard 3 random action cards", r.seat_text(*target)))
            }
            Opt::HeroFollower(yes) => {
                let Ask::WinnuFollower { follower } = a else { unreachable!("a follower option is asked by Ask::WinnuFollower") };
                let who = r.seat_text(*follower);
                if *yes {
                    ChoiceOption::labelled(format!("yes|{who}"), "follower", format!("let {who} follow"))
                } else {
                    ChoiceOption::labelled(format!("no|{who}"), "follower", format!("not {who}"))
                }
            }
            Opt::HeroCard(card) => ChoiceOption::labelled(
                card.as_str(),
                "strategy_card",
                crate::draft::strategy_card_label(ti4_content::ContentStore::embedded(), card.as_str()),
            ),
            Opt::FlockShip { index, system, unit } => {
                ChoiceOption::labelled(format!("ship|{index}"), "flock_migration_ship", format!("move {} from {}", unit.as_str(), system.as_str()))
            }
            Opt::FlockTo(system) => ChoiceOption::labelled(format!("to|{}", system.as_str()), "flock_migration_to", format!("move it to {}", system.as_str())),
            Opt::FlockCargo { index, unit } => ChoiceOption::labelled(format!("cargo|{index}"), "flock_migration_cargo", format!("carry {}", unit.as_str())),
            Opt::MahactMove(from, to) => {
                ChoiceOption::labelled(format!("{}|{}", from.as_str(), to.as_str()), "mahact_hero", format!("move the fleet in {} to {}", from.as_str(), to.as_str()))
            }
            Opt::YinSpot(system, planet) => ChoiceOption::labelled(
                format!("{}|{}", system.as_str(), planet.as_str()),
                "commit_infantry",
                format!("commit infantry to {} in {}", planet.as_str(), system.as_str()),
            ),
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
            Opt::HeroPlace(HeroUnit::Pds) => ChoiceOption::labelled("place|pds", HERO_PLACE_KIND, "place 1 PDS on a planet you control"),
            Opt::HeroPlace(HeroUnit::Mech) => ChoiceOption::labelled("place|mech", HERO_PLACE_KIND, "place 1 mech on a planet you control"),
            Opt::HeroStop => ChoiceOption::labelled("stop", HERO_STOP_KIND, "stop placing"),
            Opt::HeroPlanet(p, shown) => {
                let system = shown.system.map(|s| s.as_str());
                ChoiceOption::labelled(
                    format!("planet|{}", p.as_str()),
                    HERO_PLANET_KIND,
                    format!("{} in {}", p.as_str(), system.unwrap_or_default()),
                )
                .with_planet(p.as_str(), system)
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
            Ask::SardakkSkip => (id == "skip").then_some(Opt::SardakkSkip),
            Ask::XxchaPlanet => Planet::from_id(id).map(|p| Opt::Planet(p, Shown::default())),
            Ask::XxchaInfantry => match id {
                "remove" => Some(Opt::Remove),
                "keep" => Some(Opt::Keep),
                _ => None,
            },
            Ask::L1z1xDestination | Ask::ZeuSystem | Ask::SaarHero | Ask::GhostSwapFirst | Ask::GhostSwapSecond | Ask::UmbatSystem => System::from_id(id).map(Opt::System),
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
            // The number names a place in the list the question offered, not a ship: the id is matched against the options.
            Ask::OsphaShip => None,
            Ask::OsphaReplace { .. } => {
                let name = id.strip_prefix("replace|")?;
                [UnitBaseType::Carrier, UnitBaseType::Cruiser, UnitBaseType::Destroyer, UnitBaseType::Dreadnought, UnitBaseType::Flagship, UnitBaseType::Warsun]
                    .into_iter()
                    .find(|base| super::promissory::base_name_of(*base) == name)
                    .map(Opt::OsphaReplace)
            }
            Ask::NekroPlayer | Ask::UmbatPlayer => r.seat(id).map(Opt::Player),
            Ask::NekroPayment => match id.strip_prefix("token|") {
                Some(name) => [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().find(|pool| pool_name(*pool) == name).map(Opt::Token),
                None => (id == "discard").then_some(Opt::Discard),
            },
            Ask::RoundAgentUnit { .. } => round_target_of(r, id).map(Opt::RoundAgentUnit),
            Ask::XxchaHeroPlace => match id {
                "place|pds" => Some(Opt::HeroPlace(HeroUnit::Pds)),
                "place|mech" => Some(Opt::HeroPlace(HeroUnit::Mech)),
                "stop" => Some(Opt::HeroStop),
                _ => None,
            },
            Ask::XxchaHeroPlanet => Planet::from_id(id.strip_prefix("planet|")?).map(|p| Opt::HeroPlanet(p, Shown::default())),
            Ask::FlayeshPlanet => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::Devour(System::from_id(system)?, Planet::from_id(planet)?))
            }
            Ask::FlayeshTech => Technology::from_id(id).map(Opt::Gain),
            // The numbers name a place in the list the question offered, not a unit: the id is matched against the options.
            Ask::FlockShip | Ask::FlockCargo => None,
            Ask::MahactMove => {
                let (from, to) = id.split_once('|')?;
                Some(Opt::MahactMove(System::from_id(from)?, System::from_id(to)?))
            }
            Ask::YinCommit { .. } => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::YinSpot(System::from_id(system)?, Planet::from_id(planet)?))
            }
            Ask::FlockTo => System::from_id(id.strip_prefix("to|")?).map(Opt::FlockTo),
            Ask::NaazCard | Ask::WinnuCard => ti4_engine_core::generated::StrategyCard::from_id(id).map(Opt::HeroCard),
            Ask::WinnuFollower { follower } => {
                let who = r.seat_text(*follower);
                match id.split_once('|') {
                    Some(("yes", rest)) if rest == who => Some(Opt::HeroFollower(true)),
                    Some(("no", rest)) if rest == who => Some(Opt::HeroFollower(false)),
                    _ => None,
                }
            }
            Ask::KyverFate { .. } => match id {
                "take" => Some(Opt::KyverTake),
                "discard" => Some(Opt::KyverDiscard),
                _ => None,
            },
        }
    }
}
