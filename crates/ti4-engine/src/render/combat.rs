//! Renderer of the `combat` area: `impl Family` for `ti4_engine_core::choice::combat` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Owner: package B1.
//!
//! Ported: the sustain offer. Prompt `"cancel a hit at {system}"`, subtype `sustain_damage`; an option is
//! `sustain|{location}` (kind `sustain`, payload `{unit}`), then the generic decline worded "take the hit"
//! (payload `non_fighters_first` when the loss is bound to non-fighters).
//!
//! Not represented here: the previews (`ShipsInSystem` deltas), the `sustainers` detail and the context
//! (hits owed, target). The string resolver builds those; the dual compares ids, kinds, labels and payload.
//! `parse` cannot recover the unit type from an id without the state, so it answers `None`: the typed side
//! answers by index.

use ti4_engine_core::choice::combat::{Ask, Opt, RerollSource, Subtype, SustainSite};
use ti4_engine_core::choice::Opt as AnyOpt;
use ti4_engine_core::generated::Unit;

use super::{Family, Render, Wording};
use crate::choice::ChoiceOption;
use crate::preview::{Delta, Preview, Quantity};
use crate::generated::IdStr;

pub const SUSTAIN_KIND: &str = "sustain";

/// `ShipsInSystem` from `before` to `after` (the previews of the combat window's own questions).
fn ships(before: i64, after: i64) -> Preview {
    Preview::certain(vec![Delta::new(Quantity::ShipsInSystem, before, after)])
}

/// The renderer of the combat questions.
pub struct Fam;

/// The id suffix of a site (`space:3`, `planet:x`, `planet:x:variant:2`).
#[must_use]
pub fn location(site: SustainSite) -> String {
    match site {
        SustainSite::Space(i) => format!("space:{i}"),
        SustainSite::Planet { planet, index, variant: true } => format!("planet:{}:variant:{index}", planet.as_str()),
        SustainSite::Planet { planet, variant: false, .. } => format!("planet:{}", planet.as_str()),
    }
}

fn label(a: &Ask, site: SustainSite, unit: Unit, galvanized: bool) -> String {
    let staging = match a {
        Ask::CancelHit { staging, .. } => staging,
        _ => &false,
    };
    if let (true, SustainSite::Planet { planet, .. }) = (*staging, site) {
        let id = planet.as_str();
        let place = ti4_content::galaxy::planet(ti4_content::ContentStore::embedded(), id, ti4_model::content_types::FULL)
            .and_then(|record| record.name())
            .unwrap_or(id);
        if unit == Unit::EidolonMaximum {
            let form = if galvanized { "galvanized" } else { "plain" };
            return format!("sustain damage on Maximum on {place} ({form})");
        }
        return format!("sustain damage on {} on {place}", unit.as_str());
    }
    format!("sustain damage on {}", unit.as_str())
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_r: &Render, a: &Ask) -> String {
        match a {
            Ask::CancelHit { system, .. } => format!("cancel a hit at {}", system.as_str()),
            Ask::AnnounceRetreat { system, .. } => format!("announce a retreat from {}", system.as_str()),
            Ask::RetreatTo { .. } => "retreat to which system".to_owned(),
            Ask::Withdraw { .. } => "Skilled Retreat: withdraw to which system".to_owned(),
            Ask::AssignHit => "assign a hit".to_owned(),
            Ask::RaidDamage { .. } => "Raid Formation: choose a ship with SUSTAIN DAMAGE to become damaged".to_owned(),
            Ask::RerollDie { unit, die, .. } => format!("reroll die {} of {}", u16::from(*die) + 1, _r.code_text(*unit)),
            Ask::HeartIxth => "Heart of Ixth: add or subtract 1 from a die that was just rolled".to_owned(),
            Ask::WarFunding => "War Funding: reroll all of your opponent's dice and your misses".to_owned(),
            Ask::Wrath => "Wrath of Kenara: spend trade goods to add 1 to dice".to_owned(),
            Ask::MunitionsRoll | Ask::MunitionsStart { .. } => "spend 2 trade goods for Munitions Reserves".to_owned(),
            Ask::Supercharge => "Supercharge: exhaust to apply +1 to each combat roll this round".to_owned(),
            Ask::Ambush => "Ambush: choose up to 2 cruisers or destroyers to roll".to_owned(),
            Ask::Sacrifice { devotion: false } => "Impulse Core: destroy a cruiser or destroyer to produce 1 hit".to_owned(),
            Ask::Sacrifice { devotion: true } => "Devotion: destroy a cruiser or destroyer to produce 1 hit".to_owned(),
            Ask::Splicer => "Dimensional Splicer: produce 1 hit against your opponent's ships".to_owned(),
            Ask::DeployMech { planet } => format!("Dunlain Reaper: spend 2 resources to replace an infantry on {}", planet.as_str()),
        }
    }

    fn details(_r: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();
        // Munitions Reserves at the opening of a round: the printed ability and the cost with the goods held (the string `munitions_offer`).
        if let Ask::MunitionsStart { have } = a {
            map.insert("kind".to_owned(), "ability_offer".into());
            map.insert("ability".to_owned(), crate::faction_abilities::ability_card(ti4_content::ContentStore::embedded(), "munitions"));
            map.insert("cost".to_owned(), serde_json::json!({ "trade_goods": crate::faction_abilities::MUNITIONS_COST, "have": i64::from(*have) }));
        }
        map
    }

    fn subtype(_r: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainDamage => "sustain_damage".to_owned(),
            Subtype::AnnounceRetreat => "announce_retreat".to_owned(),
            Subtype::RetreatTo => "retreat_to".to_owned(),
            Subtype::Withdraw => "skilled_retreat_choose_system".to_owned(),
            Subtype::Casualty(code) => _r.code_text(*code),
            Subtype::Reroll(RerollSource::Relic) => "crown_of_thalnos_relic_reroll".to_owned(),
            Subtype::Reroll(RerollSource::Law) => "crown_of_thalnos_law_reroll".to_owned(),
            Subtype::Reroll(RerollSource::Munitions) => "munitions_reroll_die".to_owned(),
            Subtype::Reroll(RerollSource::FireTeam) => "fire_team_reroll".to_owned(),
            Subtype::Reroll(RerollSource::Commander) => "jolnar_commander_reroll".to_owned(),
            Subtype::HeartIxth => "heart_ixth_die_adjust".to_owned(),
            Subtype::WarFunding => "war_funding".to_owned(),
            Subtype::WrathBump => "wrath_of_kenara_bump".to_owned(),
            Subtype::MunitionsRoll => "munitions_reserves_reroll".to_owned(),
            Subtype::Supercharge => "supercharge".to_owned(),
            Subtype::Ambush => "ambush_roll".to_owned(),
            Subtype::Sacrifice { devotion: false } => "impulse_core_sacrifice".to_owned(),
            Subtype::Sacrifice { devotion: true } => "devotion_sacrifice".to_owned(),
            Subtype::Splicer => "dimensional_splicer".to_owned(),
            Subtype::DeployMech => "deploy_mech".to_owned(),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let (site, unit, galvanized) = match o {
            Opt::RaidDamage { unit } => {
                let target = match a {
                    Ask::RaidDamage { target } => r.seat_text(*target),
                    _ => "",
                };
                return ChoiceOption::labelled(unit.as_str().to_owned(), "raid_formation_damage", format!("damage 1 {} of {target}", unit.as_str()));
            }
            Opt::RerollDie { unit, entry, die, face, hits_on } => {
                let name = r.code_text(*unit);
                let mut option = ChoiceOption::labelled(
                    format!("reroll|{entry}:{die}"),
                    "reroll_die",
                    format!("reroll die {} of {name} (shows {face})", u16::from(*die) + 1),
                )
                .with("unit", name)
                .with("face", i64::from(*face));
                if let Some(on) = hits_on {
                    option = option.with("hits_on", i64::from(*on));
                }
                return option;
            }
            Opt::HeartIxth { unit, entry, die, add, single } => {
                let name = r.code_text(*unit);
                let (sign, verb, prep) = if *add { ("add", "add 1", "to") } else { ("sub", "subtract 1", "from") };
                let label = if *single {
                    format!("{verb} {prep} the die of {name}")
                } else {
                    format!("{verb} {prep} die {} of {die} of {name}", u16::from(*die) + 1)
                };
                let entry = if *single { 0 } else { *entry };
                return ChoiceOption::labelled(format!("hfix|{entry}:{die}:{sign}"), "heart_ixth", label);
            }
            Opt::UseWarFunding => return ChoiceOption::labelled("use", "promissory_note", "use War Funding"),
            Opt::UseSupercharge => return ChoiceOption::labelled("sc", "technology", "exhaust Supercharge"),
            Opt::Ambush { a, b } => {
                let (id, names) = match b {
                    Some(b) => (format!("{}+{}", a.as_str(), b.as_str()), format!("{} and {}", a.as_str(), b.as_str())),
                    None => (a.as_str().to_owned(), a.as_str().to_owned()),
                };
                return ChoiceOption::labelled(id, "ambush_roll", format!("Ambush: roll for {names}"));
            }
            Opt::Sacrifice { unit } => {
                return ChoiceOption::labelled(unit.as_str().to_owned(), "destroy_own_ship", format!("destroy 1 {} to produce 1 hit", unit.as_str()));
            }
            Opt::DeployMech => return ChoiceOption::labelled("deploy", "unit", "deploy the mech"),
            Opt::ProduceHit => {
                return ChoiceOption::labelled("produce_hit", "produce_hit", "produce 1 hit and assign it to 1 of your opponent's ships");
            }
            Opt::Kenara { count } => {
                return ChoiceOption::labelled(
                    format!("kenara|{count}"),
                    "trade_goods",
                    format!("spend {count} trade good(s): +1 to {count} die/dice, {count} more hit(s)"),
                )
                .with("count", usize::from(*count));
            }
            Opt::Munitions { choose: false, misses } => {
                return ChoiceOption::labelled("munitions", "ability", format!("reroll all {misses} missed dice"));
            }
            Opt::Munitions { choose: true, .. } => return ChoiceOption::labelled("munitions_choose", "ability", "choose which dice to reroll"),
            Opt::MunitionsStart => return ChoiceOption::labelled("munitions", "ability", "reroll this round's misses"),
            Opt::Destroy { index, unit, damaged } => {
                let tag = if *damaged { " (damaged)" } else { "" };
                return ChoiceOption::labelled(
                    format!("destroy|{index}"),
                    "casualty",
                    format!("destroy {}{tag}", unit.as_str()),
                )
                .with("unit", unit.as_str().to_owned())
                .with("damaged", *damaged);
            }
            Opt::Stay | Opt::Announce => {
                let (system, forced, own) = match a {
                    Ask::AnnounceRetreat { system, forced, ships } => (system.as_str().to_owned(), *forced, i64::from(*ships)),
                    _ => (String::new(), false, 0),
                };
                let option = if matches!(o, Opt::Stay) {
                    ChoiceOption::labelled("stay", crate::combat::RETREAT_KIND, "stay and fight").previewed(ships(own, own))
                } else {
                    let label = if forced { "retreat (forced)" } else { "announce a retreat" };
                    ChoiceOption::labelled("retreat", crate::combat::RETREAT_KIND, label).previewed(ships(own, 0))
                };
                return option.with("system", system);
            }
            Opt::RetreatTo { system, there } => {
                let fleet = match a {
                    Ask::RetreatTo { fleet, .. } => i64::from(*fleet),
                    _ => 0,
                };
                let id = system.as_str();
                return ChoiceOption::labelled(id.to_owned(), crate::combat::RETREAT_TO_KIND, format!("retreat to {id}"))
                    .with("system", id.to_owned())
                    .previewed(ships(i64::from(*there), i64::from(*there) + fleet));
            }
            Opt::Withdraw { system, there } => {
                let fleet = match a {
                    Ask::Withdraw { fleet, .. } => i64::from(*fleet),
                    _ => 0,
                };
                let id = system.as_str();
                return ChoiceOption::labelled(id.to_owned(), "skilled_retreat", format!("withdraw to {id}"))
                    .previewed(ships(i64::from(*there), i64::from(*there) + fleet));
            }
            Opt::Sustain { site, unit, galvanized } => (site, unit, galvanized),
        };
        let window = match a {
            Ask::CancelHit { window, .. } => *window,
            _ => None,
        };
        let id = match (window, site) {
            // The window names a space unit by its index.
            (Some(_), SustainSite::Space(i)) => format!("sustain|{i}"),
            _ => format!("sustain|{}", location(*site)),
        };
        let mut option = ChoiceOption::labelled(id, SUSTAIN_KIND, label(a, *site, *unit, *galvanized)).with("unit", unit.as_str().to_owned());
        if let Some(own) = window {
            option = option.previewed(ships(i64::from(own), i64::from(own)));
        }
        option
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::AnnounceRetreat { forced, .. } => match id {
                "stay" if !forced => Some(Opt::Stay),
                "retreat" => Some(Opt::Announce),
                _ => None,
            },
            Ask::RetreatTo { .. } => None,
            Ask::Withdraw { .. } => <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(id).map(|system| Opt::Withdraw { system, there: 0 }),
            Ask::RaidDamage { .. } => Some(Opt::RaidDamage { unit: <Unit as crate::generated::IdStr>::from_id(id)? }),
            Ask::Ambush => {
                let unit = |t: &str| <Unit as crate::generated::IdStr>::from_id(t);
                match id.split_once('+') {
                    Some((a, b)) => Some(Opt::Ambush { a: unit(a)?, b: Some(unit(b)?) }),
                    None => Some(Opt::Ambush { a: unit(id)?, b: None }),
                }
            }
            Ask::Sacrifice { .. } => Some(Opt::Sacrifice { unit: <Unit as crate::generated::IdStr>::from_id(id)? }),
            Ask::Splicer => (id == "produce_hit").then_some(Opt::ProduceHit),
            Ask::DeployMech { .. } => (id == "deploy").then_some(Opt::DeployMech),
            _ => None,
        }
    }

    fn wording(_r: &Render, a: &Ask, o: &AnyOpt) -> Wording {
        let non_fighters_first = match a {
            Ask::CancelHit { non_fighters_first, .. } => non_fighters_first,
            _ => &false,
        };
        let mut w = Wording::default();
        if matches!(o, AnyOpt::Decline) {
            match a {
                Ask::RerollDie { unit, .. } => {
                    w.payload.insert("unit".to_owned(), serde_json::Value::String(_r.code_text(*unit)));
                }
                Ask::HeartIxth | Ask::WarFunding | Ask::Wrath | Ask::MunitionsRoll | Ask::MunitionsStart { .. } | Ask::Supercharge | Ask::Ambush | Ask::Sacrifice { .. } | Ask::Splicer | Ask::DeployMech { .. } => {}
                _ => {
                    w.label = Some("take the hit".to_owned());
                    if let Ask::CancelHit { window: Some(own), .. } = a {
                        w.preview = Some(ships(i64::from(*own), i64::from(*own) - 1));
                    }
                    if *non_fighters_first {
                        w.payload.insert("non_fighters_first".to_owned(), serde_json::Value::Bool(true));
                    }
                }
            }
        }
        w
    }
}
