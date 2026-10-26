//! Equivalence of the typed slice with the string engine (dual run): the same position asked of
//! both, every answer compared.
//!
//! Per position: the adjacency of every corpus system (on the board or not), and for every seat,
//! every destination that can hold things and every ship of the seat, at its move value and one
//! more (a boost): the route `MovementRules::path_from_ship` returns (not only whether there is
//! one), through the adapter (`rules_from_legacy`) and through the plain typed builder where the
//! seat carries no hook state; optionally every route (`all_routes_from_ship`).

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_engine::movement::{Board, MovementRules};
use ti4_engine_core::galaxy::Adjacency;
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::names::SYSTEM_NAMES;

use crate::adapter::{
    self, MoverFlags, Seats, board_from_state, galaxy_from_legacy, is_plain, route_text,
    rules_from_legacy, ship_type, system_ix, uses_galaxy,
};
use crate::workload::{Position, ships};

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub positions: usize,
    pub adjacency_checks: usize,
    pub rules_built: usize,
    pub queries: usize,
    pub reachable: usize,
    pub route_sets: usize,
    pub plain_rules: usize,
    pub plain_queries: usize,
    pub hook_copies: usize,
    /// Rules whose board facts differ from the typed board's (a hook cleared or added a blockade).
    pub board_differs: usize,
    /// Rules with faction-hook state beyond the board facts and per-mover flags.
    pub hooked: usize,
    pub unmapped: usize,
    /// Stage 2: (seat, destination) pairs whose per-seat typed rules were compared field by field.
    pub seat_rules: usize,
    /// Stage 2: `Observed` answers compared (movable ships, can_reach, rift exits).
    pub observed_answers: usize,
    pub mismatches: Vec<String>,
}

impl Report {
    pub fn add(&mut self, other: &Self) {
        self.positions += other.positions;
        self.adjacency_checks += other.adjacency_checks;
        self.rules_built += other.rules_built;
        self.queries += other.queries;
        self.reachable += other.reachable;
        self.route_sets += other.route_sets;
        self.plain_rules += other.plain_rules;
        self.plain_queries += other.plain_queries;
        self.hook_copies += other.hook_copies;
        self.board_differs += other.board_differs;
        self.hooked += other.hooked;
        self.unmapped += other.unmapped;
        self.seat_rules += other.seat_rules;
        self.observed_answers += other.observed_answers;
        self.mismatches.extend(other.mismatches.iter().cloned());
    }

    fn miss(&mut self, what: String) {
        if self.mismatches.len() < 50 {
            self.mismatches.push(what);
        }
    }
}

/// Adjacency of every corpus system, string vs typed.
pub fn check_adjacency(
    label: &str,
    galaxy: &ti4_content::galaxy::Galaxy,
    adjacency: &Adjacency,
    report: &mut Report,
) {
    for id in SYSTEM_NAMES {
        let ix = system_ix(id).expect("a table name maps");
        let legacy: BTreeSet<&str> = galaxy.adjacent(id);
        let raw: BTreeSet<&str> = adjacency
            .of(ix)
            .iter()
            .map(|i| SYSTEM_NAMES[i])
            .collect();
        report.adjacency_checks += 1;
        if legacy != raw {
            report.miss(format!("{label}: adjacent({id}) {legacy:?} vs {raw:?}"));
        }
    }
}

/// Everything for one position. `routes`: also compare `all_routes_from_ship` (expensive).
pub fn check_position(position: &Position, routes: bool, report: &mut Report) {
    let content = ContentStore::embedded();
    let (state, galaxy, sources) = (&position.state, &position.galaxy, position.sources);
    report.positions += 1;
    let Ok(raw_galaxy) = galaxy_from_legacy(galaxy) else {
        report.unmapped += 1;
        return;
    };
    let adjacency = std::rc::Rc::new(raw_galaxy.adjacency());
    check_adjacency(&position.label, galaxy, &adjacency, report);
    let (Ok(seats), scope) = (Seats::of(state), adapter::scope(sources)) else {
        report.unmapped += 1;
        return;
    };
    let Ok(board) = board_from_state(state, &seats, sources) else {
        report.unmapped += 1;
        return;
    };
    for player in &state.players {
        let Some(seat) = seats.seat(player.id.as_str()) else {
            continue;
        };
        let fleet = ships(state, content, sources, &player.id);
        let enemy = board.enemy_ships(seat);
        let tokens = board.own_command_tokens(seat);
        for dest in galaxy.system_ids_holding_things() {
            let legacy_board = Board::for_player(state, content, sources, &player.id);
            let mut legacy =
                MovementRules::with_laws(galaxy, content, sources, dest, legacy_board, Some(state));
            ti4_engine::action_cards::apply_movement_effects(&mut legacy, state, &player.id);
            report.rules_built += 1;
            // A faction module searched a copy of the map: convert that copy.
            let own;
            let adjacency_here = if uses_galaxy(&legacy, galaxy) {
                &adjacency
            } else {
                report.hook_copies += 1;
                match galaxy_from_legacy(legacy.parts().galaxy) {
                    Ok(copy) => {
                        own = std::rc::Rc::new(copy.adjacency());
                        &own
                    }
                    Err(_) => {
                        report.unmapped += 1;
                        continue;
                    }
                }
            };
            let Ok(raw) = rules_from_legacy(&legacy, sources, adjacency_here) else {
                report.unmapped += 1;
                continue;
            };
            let Some(active) = system_ix(dest) else {
                report.unmapped += 1;
                continue;
            };
            let board_same = adapter::sys_set_lossy(
                legacy.parts().board.enemy_ships.iter().map(String::as_str),
            ) == enemy
                && adapter::sys_set_lossy(
                    legacy.parts().board.own_command_tokens.iter().map(String::as_str),
                ) == tokens;
            let hook_free = is_plain(&legacy, galaxy);
            if !board_same {
                report.board_differs += 1;
                // The only hook that edits the board facts (Aetherpassage) clears blockades: the
                // string rules' set must be a subset of the typed board's, tokens unchanged.
                let legacy_enemy = adapter::sys_set_lossy(
                    legacy.parts().board.enemy_ships.iter().map(String::as_str),
                );
                let mut both = legacy_enemy;
                both.intersect_with(&enemy);
                if both != legacy_enemy
                    || adapter::sys_set_lossy(
                        legacy.parts().board.own_command_tokens.iter().map(String::as_str),
                    ) != tokens
                {
                    report.miss(format!("{}: {} to {dest}: board facts differ beyond a cleared blockade", position.label, player.id));
                }
            }
            if !hook_free {
                report.hooked += 1;
            }
            let plain = (hook_free && board_same).then(|| {
                let mut rules = CoreRules::with_board(adjacency.clone(), scope, active, enemy, tokens);
                MoverFlags::of(&legacy).apply(&mut rules);
                rules
            });
            if plain.is_some() {
                report.plain_rules += 1;
            }
            for ship in &fleet {
                if ship.origin.as_str() == dest {
                    continue;
                }
                let (Some(origin), Ok(kind)) =
                    (system_ix(ship.origin.as_str()), ship_type(Some(&ship.type_id)))
                else {
                    report.unmapped += 1;
                    continue;
                };
                for move_value in [ship.move_value, ship.move_value + 1] {
                    let want = legacy.path_from_ship(
                        ship.origin.as_str(),
                        move_value,
                        Some(&ship.type_id),
                    );
                    let got = raw
                        .path_from_ship(origin, move_value, kind)
                        .map(|r| route_text(&r));
                    report.queries += 1;
                    if want.is_some() {
                        report.reachable += 1;
                    }
                    if want != got {
                        report.miss(format!(
                            "{}: {} {} {}->{dest} mv {move_value}: {want:?} vs {got:?}",
                            position.label, player.id, ship.type_id, ship.origin
                        ));
                    }
                    if let Some(plain) = &plain {
                        report.plain_queries += 1;
                        let got = plain
                            .path_from_ship(origin, move_value, kind)
                            .map(|r| route_text(&r));
                        if want != got {
                            report.miss(format!(
                                "{} (plain): {} {} {}->{dest} mv {move_value}: {want:?} vs {got:?}",
                                position.label, player.id, ship.type_id, ship.origin
                            ));
                        }
                    }
                    if routes && move_value == ship.move_value {
                        let want = legacy.all_routes_from_ship(
                            ship.origin.as_str(),
                            move_value,
                            Some(&ship.type_id),
                        );
                        let got: Vec<Vec<String>> = raw
                            .all_routes_from_ship(origin, move_value, kind)
                            .iter()
                            .map(|r| route_text(r))
                            .collect();
                        report.route_sets += 1;
                        if want != got {
                            report.miss(format!(
                                "{}: all routes {} {}->{dest} mv {move_value}: {} vs {} routes",
                                position.label,
                                ship.type_id,
                                ship.origin,
                                want.len(),
                                got.len()
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Switch variants of one galaxy (laws, cards, rules version 22, a token wormhole), each checked
/// for adjacency: the map changes the game makes between steps.
pub fn check_galaxy_variants(position: &Position, report: &mut Report) {
    let base = &position.galaxy;
    let mut variants = Vec::new();
    for bits in 0..32u32 {
        let mut g = base.clone();
        g.hyperlane_lanes = bits & 1 != 0;
        g.wormholes_off = bits & 2 != 0;
        g.wormholes_all_linked = bits & 4 != 0;
        g.wormhole_star_links = bits & 8 != 0;
        g.nexus_wormholes_off = bits & 16 != 0;
        variants.push((format!("switches {bits:05b}"), g));
    }
    // A gamma token on the first system, an alpha on the last, a link between the two.
    let ids = base.system_ids();
    if let (Some(first), Some(last)) = (ids.first(), ids.last()) {
        let mut g = base.clone();
        g.token_wormholes
            .entry((*first).to_owned())
            .or_default()
            .insert("GAMMA".to_owned());
        g.token_wormholes
            .entry((*last).to_owned())
            .or_default()
            .insert("ALPHA".to_owned());
        g.extra_links
            .entry((*first).to_owned())
            .or_default()
            .insert((*last).to_owned());
        g.extra_links
            .entry((*last).to_owned())
            .or_default()
            .insert((*first).to_owned());
        variants.push(("tokens".to_owned(), g.clone()));
        let mut epsilon = g.clone();
        epsilon.retain_wormhole_kinds(&["EPSILON"]);
        variants.push(("epsilon only".to_owned(), epsilon));
        let mut severed = g;
        severed.suppress_wormholes_at(first);
        variants.push(("severed".to_owned(), severed));
    }
    for (name, g) in variants {
        match galaxy_from_legacy(&g) {
            Ok(raw) => check_adjacency(
                &format!("{} {name}", position.label),
                &g,
                &raw.adjacency(),
                report,
            ),
            Err(e) => report.miss(format!("{} {name}: unmapped {e:?}", position.label)),
        }
    }
}

/// Stage 2 (docs/ENGINE_CORE.md): the per-seat rules template retargeted to each destination equals
/// the translation of `with_laws` built for that destination (field by field, with and without the
/// cards in play), and every typed `Observed` answer equals the string engine's.
pub fn check_observed(position: &Position, report: &mut Report) {
    use ti4_engine::core_adapter::SeatReach;
    let content = ContentStore::embedded();
    let (state, galaxy, sources) = (&position.state, &position.galaxy, position.sources);
    let observed = ti4_engine::choice::Observed::new(state, content, sources, Some(galaxy));
    for player in &state.players {
        let reach = [false, true].map(|effects| {
            SeatReach::new(state, content, sources, galaxy, &player.id, effects)
        });
        let fleet = ships(state, content, sources, &player.id);
        for dest in galaxy.system_ids_holding_things() {
            let dest_id = ti4_model::id::SystemId::new(dest);
            for (effects, reach) in [false, true].into_iter().zip(&reach) {
                let Some(reach) = reach else {
                    report.unmapped += 1;
                    continue;
                };
                let board = Board::for_player(state, content, sources, &player.id);
                let mut legacy =
                    MovementRules::with_laws(galaxy, content, sources, dest, board, Some(state));
                if effects {
                    ti4_engine::action_cards::apply_movement_effects(&mut legacy, state, &player.id);
                }
                let Some(adjacency) = adapter::adjacency_for(legacy.parts().galaxy) else {
                    report.unmapped += 1;
                    continue;
                };
                let want = rules_from_legacy(&legacy, sources, &adjacency).ok();
                let got = reach.rules_for(state, content, sources, &dest_id);
                report.seat_rules += 1;
                if want != got {
                    report.miss(format!(
                        "{}: seat rules {} to {dest} (effects {effects}) differ",
                        position.label, player.id
                    ));
                }
                if !effects {
                    // Observed::can_reach (laws only) for every ship origin and value.
                    for ship in &fleet {
                        for mv in [ship.move_value, ship.move_value + 1] {
                            let want = legacy.can_reach(ship.origin.as_str(), mv);
                            let got = observed.can_reach(&player.id, &ship.origin, &dest_id, mv);
                            report.observed_answers += 1;
                            if want != got {
                                report.miss(format!(
                                    "{}: can_reach {} {}->{dest} mv {mv}: {want} vs {got}",
                                    position.label, player.id, ship.origin
                                ));
                            }
                        }
                    }
                }
            }
            let want = ti4_engine::tactical::movable_into(
                state, content, sources, galaxy, &player.id, &dest_id,
            );
            let got = observed.typed_movable_into(&player.id, &dest_id);
            report.observed_answers += 1;
            if got.as_ref() != Some(&want) {
                report.miss(format!(
                    "{}: movable_into {} -> {dest}: {} vs {:?} ships",
                    position.label,
                    player.id,
                    want.len(),
                    got.map(|g| g.len())
                ));
            }
            for ship in &fleet {
                if ship.origin.as_str() == dest {
                    continue;
                }
                let Some(kind) =
                    ti4_content::units::unit_type(content, &ship.type_id, sources)
                else {
                    continue;
                };
                for boosts in [(false, false), (true, false)] {
                    let want = ti4_engine::tactical::route_rift_exits(
                        state,
                        content,
                        sources,
                        galaxy,
                        &player.id,
                        &dest_id,
                        ship.origin.as_str(),
                        &kind,
                        &ship.type_id,
                        boosts,
                    );
                    let got = observed.typed_rift_exits(
                        &player.id,
                        &ship.origin,
                        &ship.type_id,
                        boosts,
                        &dest_id,
                    );
                    report.observed_answers += 1;
                    if got != Some(want) {
                        report.miss(format!(
                            "{}: rift exits {} {} {}->{dest} {boosts:?}: {want} vs {got:?}",
                            position.label, player.id, ship.type_id, ship.origin
                        ));
                    }
                }
            }
        }
    }
}
