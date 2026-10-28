//! Renderer of the `relic` area: `impl Family` for `ti4_engine_core::choice::relic` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B5. The texts are those of `relics.rs` (`offer_dominus_orb`,
//! `crown_of_emphidia_explore_with`), byte for byte.

use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::choice::relic::{Ask, Opt, Subtype};
use ti4_engine_core::generated::Planet;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The renderer of the relic questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::DominusOrb => "The Dominus Orb: purge to move out of your own command tokens",
            Ask::CrownOfEmphidia => "The Crown of Emphidia: exhaust to explore a planet",
        }
        .to_owned()
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::DominusOrbPurgeToMove => "dominus_orb_purge_to_move",
            Subtype::CrownOfEmphidiaChoosePlanet => "crown_of_emphidia_choose_planet",
        }
        .to_owned()
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Purge => ChoiceOption::labelled("purge", "relic", "purge the Dominus Orb"),
            Opt::Planet(p, shown) => {
                let id = p.as_str();
                ChoiceOption::labelled(id, "planet", format!("explore {id}")).with_planet(id, shown.system.map(|s| s.as_str()))
            }
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::DominusOrb => (id == "purge").then_some(Opt::Purge),
            Ask::CrownOfEmphidia => Planet::from_id(id).map(|p| Opt::Planet(p, Shown::default())),
        }
    }
}
