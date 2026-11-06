//! Renderer of the `technology` area: `impl Family` for `ti4_engine_core::choice::technology` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package W12.
//!
//! The texts are those of `technology::start_turn` (Psychoarchaeology) and `technology::end_turn` (Predictive Intelligence,
//! Bio-Stims), byte for byte. The pool counts of the restack question (`Choice.details`) are the adapter's to add.

use ti4_engine_core::choice::technology::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{Planet, System, Technology};

use super::{Family, Render, Wording};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The renderer of the free technology questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::TcsTransaction { partner } => return format!("Temporal Command Suite: perform a transaction with {}", r.player(*partner)),
            Ask::Psychoarchaeology => "Psychoarchaeology: exhaust a specialty for 1 trade good",
            Ask::Restack => "Predictive Intelligence: redistribute command tokens",
            Ask::BioStims => "Bio-Stims",
            Ask::TransitDiodes { left } => return format!("Transit Diodes: redeploy ground forces ({left} left)"),
            Ask::SelfAssembly => "Self-Assembly Routines: exhaust to place 1 mech from your reinforcements on a planet you control in this system",
            Ask::ProductionBiomes => "Production Biomes: who gains 2 trade goods",
            Ask::Vortex => "Vortex: which unit type do you take",
            Ask::WormholeGenerator => "Wormhole Generator: place or move a Creuss wormhole token",
            Ask::ChaosMapping => "Chaos Mapping",
            Ask::QuantumDatahub => "Quantum Datahub Node: spend a strategy token and 3 trade goods to swap a strategy card",
            Ask::Aida { upgrades } => return format!("AI Development Algorithm: exhaust to reduce this use's combined cost by {upgrades}"),
            Ask::StartingTech { number, of, .. } => {
                return if *of > 1 { format!("choose a starting technology ({number} of {of})") } else { "choose a starting technology".to_owned() };
            }
        }
        .to_owned()
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::PsychoarchaeologyExhaustSpecialty => "psychoarchaeology_exhaust_specialty",
            Subtype::PredictiveIntelligenceRedistribute => "predictive_intelligence_redistribute",
            Subtype::BioStimsReady => "bio_stims_ready",
            Subtype::SelfAssemblyMech => "self_assembly_mech",
            Subtype::TransitDiodesRedeploy => "transit_diodes_redeploy",
            Subtype::AidaExhaustForProductionDiscount => "exhaust_for_production_discount",
            Subtype::ProductionBiomesChoosePlayer => "production_biomes_choose_player",
            Subtype::VortexTarget => "vortex_target",
            Subtype::CreussToken => "creuss_token",
            Subtype::ChaosMappingChooseSystem => "chaos_mapping_choose_system",
            Subtype::StartingTechnology => "starting_technology",
            Subtype::TcsTransaction => "tcs_transaction",
            Subtype::QuantumDatahubSwap => "quantum_datahub_swap",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            // The label is the printed name; the colour, the owners (Keleres) and the count of prerequisites are the caller's display
            // facts (`live::turn::complete`).
            (Ask::TcsTransaction { partner }, Opt::Transact) => ChoiceOption::labelled("transact", "technology", format!("transact with {}", r.player(*partner))),
            (_, Opt::Transact) => ChoiceOption::labelled("transact", "technology", "transact"),
            (_, Opt::Swap { partner, give, take }) => {
                let (partner, give, take) = (r.seat_text(*partner), give.as_str(), take.as_str());
                ChoiceOption::labelled(format!("qdn|{partner}|{give}|{take}"), crate::draft::STRATEGY_CARD_KIND, format!("give {partner} 3 trade goods and {give}, take {take}"))
                    .with("partner", partner)
                    .with("give", give)
                    .with("take", take)
            }
            (_, Opt::Tech(tech)) => {
                let id = tech.as_str();
                let name = crate::technology::name(ti4_content::ContentStore::embedded(), &ti4_model::id::TechnologyId::new(id));
                ChoiceOption::labelled(id, crate::starting_tech::KIND, name)
            }
            (_, Opt::System(system)) => {
                let system = system.as_str();
                ChoiceOption::labelled(system, "production", format!("produce one unit in {system}")).with("system", system).with("technology", "cm")
            }
            (_, Opt::Token(kind, system)) => {
                let (kind, system) = (kind.as_str(), system.as_str());
                ChoiceOption::labelled(format!("{kind}|{system}"), "creuss_token", format!("Wormhole Generator: the {kind} token into {system}")).with("system", system)
            }
            (_, Opt::Capture(owner, unit)) => {
                let (owner, unit) = (r.seat_text(*owner), unit.as_str());
                ChoiceOption::labelled(format!("{owner}|{unit}"), "vortex", format!("take a {unit} from {owner}'s reinforcements"))
            }
            (_, Opt::Player(seat)) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, "player", id)
            }
            (Ask::Aida { upgrades }, Opt::Exhaust) => ChoiceOption::labelled("exhaust", "production_discount", format!("exhaust to reduce the combined cost by {upgrades}"))
                .with("technology", "aida")
                .with("discount_offered", i64::from(*upgrades)),
            (_, Opt::Transit { from_system, from, unit, to_system, to }) => {
                let (ss, ds, planet, unit) = (from_system.as_str(), to_system.as_str(), to.as_str(), unit.as_str());
                let source = from.map_or("space", |p| p.as_str());
                ChoiceOption::labelled(format!("transit|{ss}|{source}|{unit}|{ds}|{planet}"), "transit", format!("move {unit} from {source} to {planet}"))
                    .with("source_system", ss)
                    .with("source", source)
                    .with("unit", unit)
                    .with("destination_system", ds)
                    .with("planet", planet)
            }
            (_, Opt::Exhaust) => ChoiceOption::labelled("exhaust", "production_discount", "exhaust"),
            (Ask::SelfAssembly, Opt::Planet(planet, _)) => {
                let id = planet.as_str();
                ChoiceOption::labelled(id, "self_assembly", format!("exhaust Self-Assembly Routines to place a mech on {id}"))
            }
            (Ask::Psychoarchaeology, Opt::Planet(planet, shown)) => {
                let id = planet.as_str();
                ChoiceOption::labelled(id, "ability", format!("exhaust {id}")).with("planet", id).with("technology", "pa").with_planet(id, shown.system.map(|s| s.as_str()))
            }
            (_, Opt::Planet(planet, shown)) => {
                let id = planet.as_str();
                ChoiceOption::labelled(format!("ready|planet|{id}"), "ready", format!("Bio-Stims: ready {id}"))
                    .with("planet", id)
                    .with("technology", "bs")
                    .with_planet(id, shown.system.map(|s| s.as_str()))
            }
            (_, Opt::Split { tactic, fleet, strategic }) => ChoiceOption::labelled(
                format!("{tactic}|{fleet}|{strategic}"),
                "redistribute",
                format!("tactic {tactic} / fleet {fleet} / strategy {strategic}"),
            ),
            (_, Opt::ReadyTechnology(tech)) => {
                let id = tech.as_str();
                let name = crate::technology::name(ti4_content::ContentStore::embedded(), &ti4_model::id::TechnologyId::new(id));
                ChoiceOption::labelled(format!("ready|technology|{id}"), "ready_technology", format!("Bio-Stims: ready {name}"))
                    .with("technology", id)
                    .with("bio_stims", true)
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::StartingTech { .. } => Technology::from_id(id).map(Opt::Tech),
            Ask::TcsTransaction { .. } => (id == "transact").then_some(Opt::Transact),
            Ask::ProductionBiomes => r.seat(id).map(Opt::Player),
            Ask::ChaosMapping => System::from_id(id).map(Opt::System),
            Ask::QuantumDatahub => {
                let mut parts = id.split('|');
                let (Some("qdn"), Some(partner), Some(give), Some(take), None) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { return None };
                Some(Opt::Swap { partner: r.seat(partner)?, give: ti4_engine_core::generated::StrategyCard::from_id(give)?, take: ti4_engine_core::generated::StrategyCard::from_id(take)? })
            }
            Ask::WormholeGenerator => {
                let (kind, system) = id.split_once('|')?;
                Some(Opt::Token(ti4_engine_core::generated::Wormhole::from_id(kind)?, System::from_id(system)?))
            }
            Ask::Vortex => {
                let (owner, unit) = id.split_once('|')?;
                Some(Opt::Capture(r.seat(owner)?, ti4_engine_core::generated::Unit::from_id(unit)?))
            }
            Ask::Psychoarchaeology | Ask::SelfAssembly => Planet::from_id(id).map(|p| Opt::Planet(p, ti4_engine_core::choice::agenda::Shown::default())),
            Ask::Restack => {
                let mut parts = id.split('|');
                let mut next = || parts.next()?.parse::<u8>().ok();
                let (tactic, fleet, strategic) = (next()?, next()?, next()?);
                parts.next().is_none().then_some(Opt::Split { tactic, fleet, strategic })
            }
            Ask::Aida { .. } => (id == "exhaust").then_some(Opt::Exhaust),
            Ask::TransitDiodes { .. } => {
                let mut parts = id.splitn(6, '|');
                let (Some("transit"), Some(ss), Some(source), Some(unit), Some(ds), Some(planet)) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { return None };
                Some(Opt::Transit {
                    from_system: ti4_engine_core::generated::System::from_id(ss)?,
                    from: if source == "space" { None } else { Some(Planet::from_id(source)?) },
                    unit: ti4_engine_core::generated::Unit::from_id(unit)?,
                    to_system: ti4_engine_core::generated::System::from_id(ds)?,
                    to: Planet::from_id(planet)?,
                })
            }
            Ask::BioStims => {
                if let Some(planet) = id.strip_prefix("ready|planet|") {
                    return Planet::from_id(planet).map(|p| Opt::Planet(p, ti4_engine_core::choice::agenda::Shown::default()));
                }
                id.strip_prefix("ready|technology|").and_then(Technology::from_id).map(Opt::ReadyTechnology)
            }
        }
    }

    /// "finish redeployment": the Transit Diodes question ends with a decline.
    fn wording(_: &Render, a: &Ask, o: &super::Opt) -> Wording {
        if matches!((a, o), (Ask::TransitDiodes { .. }, super::Opt::Decline)) {
            return Wording { label: Some("finish redeployment".to_owned()), ..Wording::default() };
        }
        Wording::default()
    }
}
