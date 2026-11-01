//! Renderer of the `relic` area: `impl Family` for `ti4_engine_core::choice::relic` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B5. The texts are those of `relics.rs` (`offer_dominus_orb`,
//! `crown_of_emphidia_explore_with`, `neuraloop`, `codex`, `titan_prototype`, `stellar_converter`,
//! `ask_technology`), byte for byte.

use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::choice::relic::{Ask, Opt, Subtype};
use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::generated::{ActionCard, ExploreType, Planet, Relic, Technology};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::preview::{Delta, Preview, Quantity};
use crate::generated::IdStr;

/// The name of a command pool in Fabrication's ids.
const fn pool_name(pool: Pool) -> &'static str {
    match pool {
        Pool::Tactic => "tactic",
        Pool::Fleet => "fleet",
        Pool::Strategic => "strategic",
    }
}

/// The renderer of the relic questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::DominusOrb => "The Dominus Orb: purge to move out of your own command tokens".to_owned(),
            Ask::CrownOfEmphidia => "The Crown of Emphidia: exhaust to explore a planet".to_owned(),
            Ask::Neuraloop { revealed } => format!("The Neuraloop: purge a relic to replace {}", revealed.as_str()),
            Ask::Codex => "The Codex: take which action card".to_owned(),
            Ask::TitanPrototype => "Titan Prototype: which player may build".to_owned(),
            Ask::StellarConverter => "Stellar Converter: destroy which planet".to_owned(),
            Ask::GainTechnology { .. } | Ask::GainTechnologyFrom { .. } => "gain which technology".to_owned(),
            Ask::DeviceTechnology { .. } => "research which technology with the Enigmatic Device".to_owned(),
            Ask::FragmentType { count: 1 } => "purge 1 fragment of which type".to_owned(),
            Ask::FragmentType { count } => format!("purge {count} fragments of which type"),
            Ask::TokenPool => "Fabrication: which pool gains the token".to_owned(),
        }
    }

    fn subtype(r: &Render, s: &Subtype) -> String {
        match s {
            Subtype::DominusOrbPurgeToMove => "dominus_orb_purge_to_move",
            Subtype::CrownOfEmphidiaChoosePlanet => "crown_of_emphidia_choose_planet",
            Subtype::NeuraloopChooseRelicToPurge => "neuraloop_choose_relic_to_purge",
            Subtype::CodexTakeActionCard => "codex_take_action_card",
            Subtype::TitanPrototypeChooseBuilder => "titan_prototype_choose_builder",
            Subtype::StellarConverterChooseTarget => "stellar_converter_choose_target",
            Subtype::ChooseTechnology(relic) => return format!("{}_choose_technology", relic.as_str()),
            Subtype::ChooseTechnologyFrom(source) => return format!("{}_choose_technology", r.code_text(*source)),
            Subtype::DeviceChooseTechnology => "enigmatic_device_choose_technology",
            Subtype::FragmentType => "fragment_type",
            Subtype::TokenPool => "token_pool",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Fragment(kind) => {
                let (id, count) = (kind.as_str().to_ascii_uppercase(), if let Ask::FragmentType { count } = a { *count } else { 1 });
                ChoiceOption::labelled(id.clone(), "fragment", format!("purge {count} {id}"))
            }
            Opt::Pool(pool) => {
                let id = pool_name(*pool);
                ChoiceOption::labelled(id, "pool", format!("gain 1 token in {id}"))
            }
            Opt::Purge => ChoiceOption::labelled("purge", "relic", "purge the Dominus Orb"),
            Opt::Planet(p, shown) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", format!("explore {id}")).with_planet(id, shown.system.map(|s| s.as_str()))
            }
            Opt::PurgeRelic(relic) => {
                let id = relic.as_str();
                ChoiceOption::labelled(id, "relic", format!("purge {id}"))
            }
            Opt::Take(card, shown) => {
                let (id, held) = (card.as_str(), i64::from(shown.votes));
                ChoiceOption::labelled(id, "action_card", format!("take {id}"))
                    .previewed(Preview::certain(vec![Delta::new(Quantity::ActionCardsHeld, held, held + 1)]))
            }
            Opt::Builder(seat) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, "player", format!("{id} may build"))
            }
            Opt::Destroy(p, shown) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", format!("destroy {id}")).with_planet(id, shown.system.map(|s| s.as_str()))
            }
            Opt::Tech(t) if matches!(a, Ask::DeviceTechnology { .. }) => {
                let id = t.as_str();
                let name = ti4_content::ContentStore::embedded().get(ti4_model::content_types::ContentType::Technologies, id).and_then(|record| record.text("name")).unwrap_or(id);
                ChoiceOption::labelled(id, "technology", format!("research {name}"))
            }
            Opt::Tech(t) => {
                let id = t.as_str();
                ChoiceOption::labelled(id, "technology", format!("gain {id}"))
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::DominusOrb => (id == "purge").then_some(Opt::Purge),
            Ask::CrownOfEmphidia => Planet::from_id(id).map(|p| Opt::Planet(p, Shown::default())),
            Ask::Neuraloop { .. } => Relic::from_id(id).map(Opt::PurgeRelic),
            Ask::Codex => ActionCard::from_id(id).map(|c| Opt::Take(c, Shown::default())),
            Ask::TitanPrototype => r.seat(id).map(Opt::Builder),
            Ask::StellarConverter => Planet::from_id(id).map(|p| Opt::Destroy(p, Shown::default())),
            Ask::GainTechnology { .. } | Ask::GainTechnologyFrom { .. } | Ask::DeviceTechnology { .. } => Technology::from_id(id).map(Opt::Tech),
            Ask::FragmentType { .. } => ExploreType::ALL.iter().copied().find(|k| k.as_str().eq_ignore_ascii_case(id)).map(Opt::Fragment),
            Ask::TokenPool => [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().find(|pool| pool_name(*pool) == id).map(Opt::Pool),
        }
    }
}
