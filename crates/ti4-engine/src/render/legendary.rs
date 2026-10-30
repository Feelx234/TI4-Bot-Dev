//! Renderer of the `legendary` area: `impl Family` for `ti4_engine_core::choice::legendary` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package W16. The texts are those of `legendary.rs` (`end_turn`, `pass`, `resolve`, `resolve_pass`,
//! `place_on_own_planet`), byte for byte.

use ti4_content::ContentStore;
use ti4_engine_core::choice::legendary::{Ask, Opt, Ready, Subtype, When};
use ti4_engine_core::generated::{ActionCard, Leader, Planet, Relic, SecretObjective, System, Technology, Unit};
use ti4_model::id::ActionCardId;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The renderer of the legendary planet questions.
pub struct Fam;

/// The label an ability is offered under (`END_OF_TURN` and `WHEN_YOU_PASS` of `legendary.rs`).
fn ability_label(p: Planet) -> &'static str {
    match p {
        Planet::HopesEnd => "Imperial Arms Vault",
        Planet::Primor => "The Atrament",
        Planet::Mallice | Planet::LockedMallice => "Exterrix Headquarters",
        Planet::Mirage => "Mirage Flight Academy",
        Planet::Emelpar => "The Acropolis",
        Planet::MecatolRexTe => "The Galactic Council",
        Planet::ThundersEdge => "Jupiter Brain",
        Planet::Faunus => "Maxis Central Control",
        Planet::Industrex => "Aurex Mechanica",
        Planet::Garbozia => "Dok 'N Pic's Salvage Yard",
        _ => "legendary ability",
    }
}

fn located(id: String, label: String, planet: Planet, system: Option<System>) -> ChoiceOption {
    ChoiceOption::labelled(id, "legendary", label).with_planet(planet.as_str(), system.map(|s| s.as_str()))
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::Offer { when: When::EndOfTurn } => "use a legendary planet ability".to_owned(),
            Ask::Offer { when: When::Pass } => "use a legendary planet ability as you pass".to_owned(),
            Ask::Exterrix { .. } => "Exterrix Headquarters".to_owned(),
            Ask::ArmsVault => "Imperial Arms Vault".to_owned(),
            Ask::PlaceOnPlanet { source: Planet::HopesEnd, .. } => "Imperial Arms Vault: place the mech where".to_owned(),
            Ask::PlaceOnPlanet { .. } => "The Atrament: place infantry where".to_owned(),
            Ask::PlaceFighters => "Mirage Flight Academy: place fighters where".to_owned(),
            Ask::Acropolis => "The Acropolis: ready what".to_owned(),
            Ask::Council => "The Galactic Council: discard which secret".to_owned(),
            Ask::Maxis => "Maxis Central Control: gain control of which planet".to_owned(),
            Ask::Salvage => "Dok 'N Pic's Salvage Yard: salvage which card".to_owned(),
            Ask::Aurex => "Aurex Mechanica: place which ship where".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::EndOfTurn => "legendary_end_of_turn",
            Subtype::Pass => "legendary_pass",
            Subtype::Exterrix => "legendary_exterrix",
            Subtype::ArmsVault => "legendary_arms_vault",
            Subtype::Place => "legendary_place",
            Subtype::Acropolis => "legendary_acropolis",
            Subtype::GalacticCouncil => "legendary_galactic_council",
            Subtype::Maxis => "legendary_maxis",
            Subtype::Salvage => "legendary_salvage",
            Subtype::Aurex => "legendary_aurex",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Use(p) => ChoiceOption::labelled(p.as_str(), "legendary", ability_label(*p)),
            Opt::Goods => ChoiceOption::labelled("goods", "legendary", "gain 2 trade goods"),
            Opt::Convert => {
                let commodities = if let Ask::Exterrix { commodities } = a { *commodities } else { 0 };
                ChoiceOption::labelled("convert", "legendary", format!("convert {commodities} commodities to trade goods"))
            }
            Opt::Mech => ChoiceOption::labelled("mech", "legendary", "place 1 mech"),
            Opt::Card => ChoiceOption::labelled("card", "legendary", "draw 1 action card"),
            Opt::OnPlanet(planet, shown) => {
                let system = shown.system.map_or("", |s| s.as_str());
                let unit = if let Ask::PlaceOnPlanet { unit, .. } = a { unit.as_str() } else { "" };
                located(format!("{system}|{}", planet.as_str()), format!("place on {}", planet.as_str()), *planet, shown.system).with("unit", unit)
            }
            Opt::InSystem(s) => ChoiceOption::labelled(s.as_str(), "legendary", format!("place fighters in {}", s.as_str())),
            Opt::Ready(what, shown) => match what {
                Ready::Planet(p) => located(format!("planet|{}", p.as_str()), format!("ready {}", p.as_str()), *p, shown.system),
                Ready::Technology(t) => ChoiceOption::labelled(format!("technology|{}", t.as_str()), "legendary", format!("ready {}", t.as_str())),
                Ready::Relic(r) => ChoiceOption::labelled(format!("relic|{}", r.as_str()), "legendary", format!("ready {}", r.as_str())),
                Ready::Legendary(p) => ChoiceOption::labelled(format!("legendary|{}", p.as_str()), "legendary", format!("ready the {} ability", p.as_str())),
                Ready::Leader(l) => ChoiceOption::labelled(format!("leader|{}", l.as_str()), "legendary", format!("ready {}", l.as_str())),
            },
            Opt::DiscardSecret(s) => ChoiceOption::labelled(s.as_str(), "legendary", "discard this secret"),
            Opt::Take(system, planet) => located(
                format!("{}|{}", system.as_str(), planet.as_str()),
                format!("gain control of {}", planet.as_str()),
                *planet,
                Some(*system),
            ),
            Opt::Salvage(card) => {
                let name = crate::action_cards::name_of(ContentStore::embedded(), &ActionCardId::new(card.as_str()));
                ChoiceOption::labelled(card.as_str(), "legendary", format!("salvage {name}"))
            }
            Opt::Ship(unit, system) => {
                ChoiceOption::labelled(format!("{}|{}", unit.as_str(), system.as_str()), "legendary", format!("place a {} in {}", unit.as_str(), system.as_str()))
            }
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        use ti4_engine_core::choice::agenda::Shown;
        match a {
            Ask::Offer { .. } => Planet::from_id(id).map(Opt::Use),
            Ask::Exterrix { .. } => match id {
                "goods" => Some(Opt::Goods),
                "convert" => Some(Opt::Convert),
                _ => None,
            },
            Ask::ArmsVault => match id {
                "mech" => Some(Opt::Mech),
                "card" => Some(Opt::Card),
                _ => None,
            },
            Ask::PlaceOnPlanet { .. } => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::OnPlanet(Planet::from_id(planet)?, Shown { votes: 0, system: System::from_id(system) }))
            }
            Ask::PlaceFighters => System::from_id(id).map(Opt::InSystem),
            Ask::Acropolis => {
                let (kind, rest) = id.split_once('|')?;
                let none = Shown::default();
                Some(match kind {
                    "planet" => Opt::Ready(Ready::Planet(Planet::from_id(rest)?), none),
                    "technology" => Opt::Ready(Ready::Technology(Technology::from_id(rest)?), none),
                    "relic" => Opt::Ready(Ready::Relic(Relic::from_id(rest)?), none),
                    "legendary" => Opt::Ready(Ready::Legendary(Planet::from_id(rest)?), none),
                    "leader" => Opt::Ready(Ready::Leader(Leader::from_id(rest)?), none),
                    _ => return None,
                })
            }
            Ask::Council => SecretObjective::from_id(id).map(Opt::DiscardSecret),
            Ask::Maxis => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::Take(System::from_id(system)?, Planet::from_id(planet)?))
            }
            Ask::Salvage => ActionCard::from_id(id).map(Opt::Salvage),
            Ask::Aurex => {
                let (unit, system) = id.split_once('|')?;
                Some(Opt::Ship(Unit::from_id(unit)?, System::from_id(system)?))
            }
        }
    }
}
