//! From `tactical_live.rs` of the legacy crate: the display facts of a tactical question (copies).

use ti4_engine_core::kernel::ability::TacticalGap;

/// The prompt of the Minister of Peace's offer to discard its law (`Game::minister_of_peace`).
pub const PEACE_PROMPT: &str = "Minister of Peace: discard to end the active player's turn";

/// The words of a position the native tactical action does not run (`Unknown::UnportedTactical`).
#[must_use]
pub const fn gap_text(gap: TacticalGap) -> &'static str {
    match gap {
        TacticalGap::NoProgress => "the tactical action made no progress (a stage that neither asks nor moves on)",
        TacticalGap::ActivationHook => "a hook of the activation that the native chain does not carry",
        TacticalGap::NovaSeed => "Nova Seed replaced a tile while the movement step closed",
        TacticalGap::NovaSeedCargo => "Nova Seed would destroy another player's ground forces carried in the space area (the string announces them with the system in the place of a planet)",
        TacticalGap::CannonHits => "a hit of the space cannon offense that needs a question",
        TacticalGap::ExplorationAtTheEnd => "the Crown of Emphidia or Dark Energy Tap explores at the end of the action (no native planet exploration window yet)",
        TacticalGap::NoGalaxy => "the map slice holds no galaxy",
    }
}

use serde_json::json;
use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Opt as COpt};
use ti4_engine_core::driver::windows::tactical::fleet_limit;
use ti4_engine_core::generated::{Unit, System};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::fleet::{self, Arrival};
use ti4_engine_core::query::reach::{scope_of, SeatFleet, SeatReach};
use ti4_engine_core::state::{CoreState, TStage, Window as OpenWindow, WindowKind};

use crate::wire::choice::Choice;
use crate::wire::preview::{Delta, Preview, Quantity};

/// The display facts of a tactical question that only the position knows (`Choice.details` and the option previews), computed from the core
/// alone: the string builds them in `activation_options_with`, `preview_moves` and `Game::tactical_choice`.
pub fn complete(core: &CoreState, k: &Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    let Ask::Tactical(ask) = choice.ask else { return };
    match ask {
        TAsk::Activate { seat } => {
            let tokens = core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.tactic_tokens));
            let spend = Preview::certain(vec![Delta::new(Quantity::TacticTokens, tokens, tokens - 1)]);
            for option in &mut rendered.options {
                option.preview = Some(spend.clone());
            }
        }
        TAsk::Move { seat } => move_details(core, k, seat, choice, rendered),
        TAsk::Load { seat, .. } => {
            let Some(OpenWindow::Tactical(t)) = core.pending().window(WindowKind::Tactical) else { return };
            let TStage::Loading { path, .. } = &t.stage else { return };
            let exits = rifts_on(core, k, seat, path.as_slice());
            if exits > 0 {
                rendered.details.insert("rift_exits".to_owned(), exits.into());
            }
        }
        // The string's removal question carries no context (`render::tactical`).
        TAsk::Remove { .. } => rendered.context = None,
        // The hooks of the activation carry no display facts of their own.
        TAsk::Hook { .. } | TAsk::Scanlink { .. } => {}
    }
}

/// The gravity rifts a route exits for `seat` (`Game::route_rift_exits`): none when the ship ignores anomalies or rifts.
fn rifts_on(core: &CoreState, k: &Kernel, seat: ti4_engine_core::Seat, path: &[System]) -> usize {
    let Some(active) = core.meta().active_system else { return 0 };
    let Some(reach) = SeatReach::new(core, k, seat, true) else { return 0 };
    let rules = reach.rules_for(core, k, active);
    if rules.anomalies_ignored || rules.rifts_ignored {
        0
    } else {
        rules.rifts_exited(path)
    }
}

/// The routes of one option as `tactical::MoveRoute` serialises them (a false flag and an empty list are left out).
fn route_json(option_id: &str, routes: &[ti4_engine_core::rules::tactical::routes::Route]) -> serde_json::Value {
    let ids = |list: &[System]| list.iter().map(|s| <System as crate::generated::IdStr>::as_str(*s).to_owned()).collect::<Vec<_>>();
    routes
        .iter()
        .enumerate()
        .map(|(n, r)| {
            let mut m = serde_json::Map::new();
            m.insert("path".to_owned(), json!(ids(&r.path)));
            m.insert("answer".to_owned(), json!(if n == 0 { option_id.to_owned() } else { format!("{option_id}|r{n}") }));
            m.insert("default".to_owned(), json!(r.default));
            if r.gd {
                m.insert("gravity_drive".to_owned(), json!(true));
            }
            if r.ionian {
                m.insert("ionian".to_owned(), json!(true));
            }
            if !r.rifts.is_empty() {
                m.insert("rifts".to_owned(), json!(ids(&r.rifts)));
            }
            if r.wormholes != 0 {
                m.insert("wormholes".to_owned(), json!(r.wormholes));
            }
            if !r.anomalies.is_empty() {
                m.insert("anomalies".to_owned(), json!(ids(&r.anomalies)));
            }
            if !r.pickups.is_empty() {
                m.insert("pickups".to_owned(), json!(ids(&r.pickups)));
            }
            serde_json::Value::Object(m)
        })
        .collect::<Vec<_>>()
        .into()
}
/// What a ship hull is keyed by in the `movable` count: its type, whether it is damaged, its origin, and two flags of its movement.
type HullKey = (Unit, bool, System, bool, bool);

/// `tactical::preview_moves` and the `movable` count of `movement_options`.
fn move_details(core: &CoreState, k: &Kernel, seat: ti4_engine_core::Seat, choice: &CoreChoice, rendered: &mut Choice) {
    let Some(active) = core.meta().active_system else { return };
    let scope = scope_of(k);
    let limit = i64::from(fleet_limit(core, k, seat));
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(core, k, seat, u);
    let before = fleet::standing(core, scope, seat, active, limit, None, &half);
    let Some(reach) = SeatReach::new(core, k, seat, true) else { return };
    let rules = reach.rules_for(core, k, active);
    // `movable`: how many hulls each option stands for.
    let mut hulls = serde_json::Map::new();
    let hull = |origin: System, index: u16| ti4_engine_core::query::board::space_units(core, origin).iter().filter(|p| p.owner() == seat).nth(usize::from(index)).copied();
    let mut keys: Vec<(HullKey, String)> = Vec::new();
    let ids: Vec<String> = rendered.options.iter().map(|o| o.id.clone()).collect();
    for (i, opt) in choice.opts.iter().enumerate() {
        if let COpt::Tactical(TOpt::Move { origin, index, kind, damaged, gd, ionian, .. }) = opt {
            let _ = index;
            keys.push(((*kind, *damaged, *origin, *gd, *ionian), ids[i].clone()));
        }
    }
    for found in SeatFleet::new(core, k, seat).movable(&rules) {
        let Some(piece) = hull(found.origin, found.index) else { continue };
        let key = (found.kind, piece.sustained(), found.origin, found.gravity_drive, found.ionian);
        if let Some((_, id)) = keys.iter().find(|(kk, _)| *kk == key) {
            let n = hulls.get(id).and_then(serde_json::Value::as_u64).unwrap_or(0);
            hulls.insert(id.clone(), json!(n + 1));
        }
    }
    if !hulls.is_empty() {
        rendered.details.insert("movable".to_owned(), hulls.into());
    }
    let mut rift_map = serde_json::Map::new();
    for (i, opt) in choice.opts.iter().enumerate() {
        let COpt::Tactical(TOpt::Move { origin, kind, gd, ionian, .. }) = opt else { continue };
        let after = fleet::standing(core, scope, seat, active, limit, Some(Arrival { kind: *kind, count: 1, in_space: true }), &half);
        let exits = ti4_engine_core::query::reach::effective_move_value(core, k, seat, *origin, *kind, (*gd, *ionian))
            .and_then(|value| rules.path_from_ship(*origin, value, Some(*kind)))
            .map_or(0, |path| if rules.anomalies_ignored || rules.rifts_ignored { 0 } else { rules.rifts_exited(&path) });
        let preview = if exits > 0 {
            rift_map.insert(ids[i].clone(), json!(exits));
            Preview::unknown("gravity-rift survival is unresolved")
        } else {
            Preview::certain(vec![
                Delta::new(Quantity::FleetSupplyHeadroom, before.fleet_headroom(), after.fleet_headroom()),
                Delta::new(Quantity::CapacityFree, before.capacity_free(), after.capacity_free()),
            ])
        };
        rendered.options[i].preview = Some(preview);
    }
    let mut supply = json!({
        "used": before.fleet_charged,
        "limit": before.fleet_limit,
        "ships": before.ships,
        "capacity": before.transport,
        "carried": before.carried,
        "fighters": before.fighters,
        "fighter_support": before.fighter_support,
        "fighters_upgraded": before.upgraded,
        "fighters_half": before.half_weight,
    });
    let armada = ti4_engine_core::rules::abilities::armada_bonus(core, seat);
    if armada > 0 {
        supply["armada"] = json!(armada);
    }
    if !rift_map.is_empty() {
        rendered.details.insert("rift_exits".to_owned(), rift_map.into());
    }
    if core.meta().rules_at_least(ti4_model::state::RULES_MOVEMENT_ROUTES) {
        let mut listed = serde_json::Map::new();
        for (i, opt) in choice.opts.iter().enumerate() {
            let COpt::Tactical(TOpt::Move { origin, index, kind, gd, ionian, .. }) = opt else { continue };
            let routes = ti4_engine_core::rules::tactical::routes::ship_routes(core, k, seat, *origin, *index, *kind, (*gd, *ionian));
            if routes.len() > 1 {
                listed.insert(ids[i].clone(), route_json(&ids[i], &routes));
            }
        }
        if !listed.is_empty() {
            rendered.details.insert("routes".to_owned(), listed.into());
        }
    }
    rendered.details.insert("fleet_supply".to_owned(), supply);
}

