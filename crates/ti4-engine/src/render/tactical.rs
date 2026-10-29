//! Renderer of the `tactical` area: `impl Family` for `ti4_engine_core::choice::tactical` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package A2b.
//!
//! Byte for byte what `tactical::activation_options_with`, `tactical::movement_options` and
//! `transit::CargoWindow::pending_choice` list: the ids, kinds, labels and payloads. Not represented here: the
//! option previews and the question details (`routes`, `fleet_supply`, `movable`, `rift_exits`); the dual run
//! takes them from the recorded string question (item A2b-12 of docs/todo/A2b.md produces them from the core).

use ti4_engine_core::choice::tactical::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{Planet, System, Unit};

use super::{Family, Render};
use crate::choice::{ChoiceOption, DECLINE_KIND};
use crate::generated::IdStr;

/// The renderer of the tactical questions.
pub struct Fam;

/// The kind of an activation option (`tactical::ACTIVATE_KIND`).
const ACTIVATE_KIND: &str = "activate";
/// The kind of a move option (`tactical::MOVE_KIND`).
const MOVE_KIND: &str = "move";
/// The kind of a pickup option (`transit::LOAD_KIND`).
const LOAD_KIND: &str = "load";

fn system_id(s: System) -> &'static str {
    <System as IdStr>::as_str(s)
}

fn unit_id(u: Unit) -> &'static str {
    <Unit as IdStr>::as_str(u)
}

/// The verb of a move id for its boosts (`movement_options`).
#[must_use]
pub const fn move_verb(gd: bool, ionian: bool) -> &'static str {
    match (gd, ionian) {
        (false, false) => "move",
        (true, false) => "move_gd",
        (false, true) => "move_ion",
        (true, true) => "move_gd_ion",
    }
}

/// The id of a move option, route suffix included (`tactical::route_suffix` reads it back).
#[must_use]
pub fn move_id(origin: System, index: u16, gd: bool, ionian: bool, route: u8) -> String {
    let base = format!("{}|{}|{index}", move_verb(gd, ionian), system_id(origin));
    if route == 0 { base } else { format!("{base}|r{route}") }
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::Activate { .. } => "activate a system".to_owned(),
            Ask::Move { .. } => "movement".to_owned(),
            Ask::Load { ship, free, .. } => format!("load {} ({free} free)", unit_id(*ship)),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::ActivateSystem => "activate_system",
            Subtype::MovementStep => "movement_step",
            Subtype::LoadCargo => "load_cargo",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Activate(system) => {
                let id = system_id(*system);
                ChoiceOption::labelled(id, ACTIVATE_KIND, format!("activate {id}"))
            }
            Opt::Move { origin, index, kind, damaged, capacity, gd, ionian, route } => {
                let spent = match (gd, ionian) {
                    (false, false) => "",
                    (true, false) => " using Gravity Drive",
                    (false, true) => " using the Ionian Fuel Refinery",
                    (true, true) => " using Gravity Drive and the Ionian Fuel Refinery",
                };
                let damage = if *damaged { " (damaged)" } else { "" };
                let origin_id = system_id(*origin);
                let mut option = ChoiceOption::labelled(
                    move_id(*origin, *index, *gd, *ionian, *route),
                    MOVE_KIND,
                    format!("move {}{damage} from {origin_id}{spent}", unit_id(*kind)),
                )
                .with("origin", origin_id)
                .with("unit", unit_id(*kind))
                .with("damaged", *damaged)
                .with("capacity", i64::from(*capacity))
                .with("gravity_drive", *gd);
                if *ionian {
                    option = option.with("ionian", true);
                }
                option
            }
            Opt::DoneMoving => ChoiceOption::labelled("done_moving", DECLINE_KIND, "finish movement"),
            Opt::Load { index, kind, source, pickup, damaged, galvanized, remaining, ground, fighters } => {
                let Ask::Load { origin, .. } = a else { panic!("a pickup in a question of another kind") };
                let where_from = source.map_or("space", |p| <Planet as IdStr>::as_str(p));
                let source_value = source.map_or(serde_json::Value::Null, |p| <Planet as IdStr>::as_str(p).into());
                ChoiceOption::labelled(format!("load|{index}"), LOAD_KIND, format!("load {} from {where_from}", unit_id(*kind)))
                    .with("unit", unit_id(*kind))
                    .with("source", source_value)
                    .with("pickup_system", system_id(*pickup))
                    .with("damaged", *damaged)
                    .with("galvanized", *galvanized)
                    .with("capacity_remaining", i64::from(*remaining))
                    .with("loaded_ground", usize::from(*ground))
                    .with("loaded_fighters", usize::from(*fighters))
                    .with("system", system_id(*origin))
            }
            Opt::DoneLoading { ground, fighters, available } => {
                let Ask::Load { origin, .. } = a else { panic!("a pickup in a question of another kind") };
                ChoiceOption::labelled("done_loading", DECLINE_KIND, "carry nothing further")
                    .with("loaded_ground", usize::from(*ground))
                    .with("loaded_fighters", usize::from(*fighters))
                    .with("ground_available", usize::from(*available))
                    .with("system", system_id(*origin))
            }
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::Activate { .. } => <System as IdStr>::from_id(id).map(Opt::Activate),
            // A hull, a pickup and a close of the hold carry facts of the position: the dual matches the id against
            // the rendered options of the very question (`dual_window::drive_positioned`).
            Ask::Move { .. } => (id == "done_moving").then_some(Opt::DoneMoving),
            Ask::Load { .. } => None,
        }
    }
}
