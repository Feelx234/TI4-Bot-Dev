//! Renderer of the `invasion` area: `impl Family` for `ti4_engine_core::choice::invasion` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Owner: package B2.
//!
//! Ported: the commit question. Prompt `"commit ground forces in {system}"`, subtype `commit_ground_forces`;
//! an option is `commit|{index}|{planet}` (kind `commit`, label `land {unit}[ (damaged)] on {planet}`,
//! payload `planet`, `unit`, `damaged`), then `done_committing` (kind `decline`).
//!
//! Not represented here: the previews (the ground forces on the planet before and after) and the options
//! that carry an origin (`from_system`, `from_planet`: a faction's commit candidates). The string resolver
//! builds those; the dual compares ids, kinds, labels and payload and leaves a list with an origin to the
//! string answer.

use ti4_engine_core::choice::invasion::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub const COMMIT_KIND: &str = "commit";

/// The renderer of the invasion questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::CommitGround { system } => format!("commit ground forces in {}", system.as_str()),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::CommitGroundForces => "commit_ground_forces".to_owned(),
        }
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Commit { index, planet, unit, damaged } => {
                let mut label = format!("land {}", unit.as_str());
                if *damaged {
                    label.push_str(" (damaged)");
                }
                ChoiceOption::labelled(
                    format!("commit|{index}|{}", planet.as_str()),
                    COMMIT_KIND,
                    format!("{label} on {}", planet.as_str()),
                )
                .with("planet", planet.as_str().to_owned())
                .with("unit", unit.as_str().to_owned())
                .with("damaged", *damaged)
            }
            Opt::DoneCommitting => ChoiceOption::labelled(
                "done_committing",
                crate::choice::DECLINE_KIND,
                "commit no more ground forces",
            ),
        }
    }

    fn parse(_: &Render, _: &Ask, id: &str) -> Option<Opt> {
        if id == "done_committing" {
            return Some(Opt::DoneCommitting);
        }
        // The unit type and its damage are not in the id; the typed side answers by index.
        None
    }
}
