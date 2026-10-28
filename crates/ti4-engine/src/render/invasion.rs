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
            Ask::AssignGroundHit { planet, .. } => format!("assign a hit on {}", planet.as_str()),
            Ask::RemoveCustodians { cost: 0, .. } => "remove the custodians token (no influence required)".to_owned(),
            Ask::RemoveCustodians { cost, .. } => format!("spend {cost} influence to remove the custodians token"),
            Ask::FightRound { planet, .. } => format!("fight a round on {}", planet.as_str()),
            Ask::StartNextCombat { planet, .. } => format!("start another ground combat on {}", planet.as_str()),
            Ask::BombardTarget { planet, hits, .. } => {
                format!("whose units on {} take the bombardment's next hits ({hits} hits)", planet.as_str())
            }
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::CommitGroundForces => "commit_ground_forces".to_owned(),
            Subtype::AssignGroundCasualty => "assign_ground_casualty".to_owned(),
            Subtype::RemoveCustodians => "remove_custodians".to_owned(),
            Subtype::FightGroundCombatRound => "fight_ground_combat_round".to_owned(),
            Subtype::StartNextGroundCombat => "start_next_ground_combat".to_owned(),
            Subtype::BombardmentTarget => "bombardment_target".to_owned(),
        }
    }

    fn option(r: &Render, ask: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Commit { index, planet, unit, damaged, from } => {
                let mut label = format!("land {}", unit.as_str());
                if *damaged {
                    label.push_str(" (damaged)");
                }
                let option = ChoiceOption::labelled(
                    format!("commit|{index}|{}", planet.as_str()),
                    COMMIT_KIND,
                    format!("{label} on {}", planet.as_str()),
                )
                .with("planet", planet.as_str().to_owned())
                .with("unit", unit.as_str().to_owned())
                .with("damaged", *damaged);
                match from {
                    Some((system, planet)) => option.with("from_system", system.as_str().to_owned()).with("from_planet", planet.as_str().to_owned()),
                    None => option,
                }
            }
            Opt::DestroyGround { index, unit, damaged, staging } => {
                let label = if *staging {
                    format!("destroy {} ({})", unit.as_str(), if *damaged { "damaged" } else { "undamaged" })
                } else {
                    format!("destroy {}", unit.as_str())
                };
                ChoiceOption::labelled(format!("destroy|{index}"), crate::invasion::GROUND_CASUALTY_KIND, label)
                    .with("unit", unit.as_str().to_owned())
                    .with("damaged", *damaged)
            }
            Opt::FightNext { next } => {
                let planet = if let Ask::StartNextCombat { planet, .. } = ask { planet.as_str() } else { "" };
                let who = r.seat_text(*next);
                ChoiceOption::labelled(format!("fight|{who}"), crate::invasion::GROUND_CASUALTY_KIND, format!("fight {who} on {planet}"))
            }
            Opt::LeaveCoexisting { next } => {
                let planet = if let Ask::StartNextCombat { planet, .. } = ask { planet.as_str() } else { "" };
                ChoiceOption::labelled("decline", crate::choice::DECLINE_KIND, format!("leave {} coexisting on {planet}", r.seat_text(*next)))
            }
            Opt::BombardTarget { seat } => {
                let who = r.seat_text(*seat);
                let mut o = ChoiceOption::labelled(who.to_owned(), "bombardment_target", format!("{who}'s units"));
                if let Ask::BombardTarget { system, planet, .. } = ask {
                    o = o.with("system", system.as_str().to_owned()).with("planet", planet.as_str().to_owned());
                }
                o
            }
            Opt::KeepCustodians => ChoiceOption::labelled("no", crate::choice::DECLINE_KIND, "leave it"),
            Opt::RemoveCustodians => ChoiceOption::labelled("yes", "custodians", "remove it for a victory point"),
            Opt::FightRound { planet } => {
                ChoiceOption::labelled("fight", crate::invasion::GROUND_CASUALTY_KIND, format!("fight a round on {}", planet.as_str()))
            }
            Opt::Deploy { planet } => ChoiceOption::labelled(
                format!("deploy_commit|{}", planet.as_str()),
                COMMIT_KIND,
                format!("deploy a {} on {}", crate::factions::crimson::MECH, planet.as_str()),
            )
            .with("planet", planet.as_str().to_owned())
            .with("unit", crate::factions::crimson::MECH),
            Opt::DoneCommitting => ChoiceOption::labelled(
                "done_committing",
                crate::choice::DECLINE_KIND,
                "commit no more ground forces",
            ),
        }
    }

    fn parse(r: &Render, ask: &Ask, id: &str) -> Option<Opt> {
        if let Ask::BombardTarget { .. } = ask {
            return r.seat(id).map(|seat| Opt::BombardTarget { seat });
        }
        if let Ask::StartNextCombat { next, .. } = ask {
            return match id {
                "decline" => Some(Opt::LeaveCoexisting { next: *next }),
                _ if id.strip_prefix("fight|").is_some_and(|who| r.seat(who) == Some(*next)) => Some(Opt::FightNext { next: *next }),
                _ => None,
            };
        }
        match (ask, id) {
            (_, "done_committing") => return Some(Opt::DoneCommitting),
            (Ask::RemoveCustodians { .. }, "no") => return Some(Opt::KeepCustodians),
            (Ask::RemoveCustodians { .. }, "yes") => return Some(Opt::RemoveCustodians),
            (Ask::FightRound { planet, .. }, "fight") => return Some(Opt::FightRound { planet: *planet }),
            _ => {}
        }
        // The unit type and its damage are not in the id; the typed side answers by index.
        None
    }
}
