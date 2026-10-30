//! Renderer of the `technology` area: `impl Family` for `ti4_engine_core::choice::technology` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package W12.
//!
//! The texts are those of `technology::start_turn` (Psychoarchaeology) and `technology::end_turn` (Predictive Intelligence,
//! Bio-Stims), byte for byte. The pool counts of the restack question (`Choice.details`) are the adapter's to add.

use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::choice::technology::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{Planet, Technology};

use super::{Family, Render, Wording};
use crate::choice::{ChoiceOption, DECLINE_KIND};
use crate::generated::IdStr;

/// The renderer of the free technology questions.
pub struct Fam;

/// The pool's name in the restack ids (`tactic|fleet`; the strategy pool is `strategy` here).
const fn pool(p: Pool) -> &'static str {
    match p {
        Pool::Tactic => "tactic",
        Pool::Fleet => "fleet",
        Pool::Strategic => "strategy",
    }
}

fn pool_of(text: &str) -> Option<Pool> {
    [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().find(|p| pool(*p) == text)
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::Psychoarchaeology => "Psychoarchaeology: exhaust a specialty for 1 trade good",
            Ask::Restack => "Predictive Intelligence: redistribute command tokens",
            Ask::BioStims => "Bio-Stims",
            Ask::SelfAssembly => "Self-Assembly Routines: exhaust to place 1 mech from your reinforcements on a planet you control in this system",
            Ask::Aida { upgrades } => return format!("AI Development Algorithm: exhaust to reduce this use's combined cost by {upgrades}"),
        }
        .to_owned()
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::PsychoarchaeologyExhaustSpecialty => "psychoarchaeology_exhaust_specialty",
            Subtype::PredictiveIntelligenceRedistribute => "predictive_intelligence_redistribute",
            Subtype::BioStimsReady => "bio_stims_ready",
            Subtype::SelfAssemblyMech => "self_assembly_mech",
            Subtype::AidaExhaustForProductionDiscount => "exhaust_for_production_discount",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::Aida { upgrades }, Opt::Exhaust) => ChoiceOption::labelled("exhaust", "production_discount", format!("exhaust to reduce the combined cost by {upgrades}"))
                .with("technology", "aida")
                .with("discount_offered", i64::from(*upgrades)),
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
            (_, Opt::Move { from, to }) => {
                let (source, destination) = (pool(*from), pool(*to));
                ChoiceOption::labelled(format!("{source}|{destination}"), "redistribute", format!("move 1 token from {source} to {destination}"))
            }
            (_, Opt::ReadyTechnology(tech)) => {
                let id = tech.as_str();
                let name = crate::technology::name(ti4_content::ContentStore::embedded(), &ti4_model::id::TechnologyId::new(id));
                ChoiceOption::labelled(format!("ready|technology|{id}"), "ready_technology", format!("Bio-Stims: ready {name}"))
                    .with("technology", id)
                    .with("bio_stims", true)
            }
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::Psychoarchaeology | Ask::SelfAssembly => Planet::from_id(id).map(|p| Opt::Planet(p, ti4_engine_core::choice::agenda::Shown::default())),
            Ask::Restack => {
                let (from, to) = id.split_once('|')?;
                Some(Opt::Move { from: pool_of(from)?, to: pool_of(to)? })
            }
            Ask::Aida { .. } => (id == "exhaust").then_some(Opt::Exhaust),
            Ask::BioStims => {
                if let Some(planet) = id.strip_prefix("ready|planet|") {
                    return Planet::from_id(planet).map(|p| Opt::Planet(p, ti4_engine_core::choice::agenda::Shown::default()));
                }
                id.strip_prefix("ready|technology|").and_then(Technology::from_id).map(Opt::ReadyTechnology)
            }
        }
    }

    /// "finish redistribution": the restack question ends with a `done` option of the decline kind.
    fn wording(_: &Render, a: &Ask, o: &super::Opt) -> Wording {
        if matches!((a, o), (Ask::Restack, super::Opt::Done)) {
            return Wording { kind: Some(DECLINE_KIND.to_owned()), label: Some("finish redistribution".to_owned()), ..Wording::default() };
        }
        Wording::default()
    }
}
