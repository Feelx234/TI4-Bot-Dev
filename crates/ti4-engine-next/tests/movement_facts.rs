//! D2-8f: the movement facts of the host's plan view on the typed core (`queries::movement::MoveView`) against the legacy string helpers they
//! replace: `MovementRules::with_laws` + `Board::for_player` + `action_cards::apply_movement_effects` (the path, may_depart, nebula_caps, the
//! rolls), `transit::rifts_exited`, `tactical::ship_routes` and `transit::CargoWindow`. Legacy is the oracle only (never called by the product).
//! Positions: played games (random deciders on the legacy engine, `surface_common`), template maps with hyperlanes, every faction armed, and
//! constructed variants (laws, cards, tokens, command tokens, enemy fleets).
mod surface_common;


use surface_common::{Pos, content, positions};
use ti4_content::galaxy::Galaxy;
use ti4_content::units::catalogue;
use ti4_engine as thin;
use ti4_engine::queries::movement::{MoveView, Pool};
use ti4_engine_legacy as old;
use ti4_model::content_types::{POK, SourceSet};
use ti4_model::id::{PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

#[derive(Default, Debug)]
struct Counts {
    views: usize,
    ships: usize,
    paths: usize,
    rifts: usize,
    routes: usize,
    multi_routes: usize,
    pools: usize,
    departures_blocked: usize,
    unrolled: usize,
}

fn pool_key(choice: Option<old::choice::Choice>) -> Vec<Pool> {
    let Some(choice) = choice else { return Vec::new() };
    choice
        .options
        .iter()
        .filter(|option| option.kind == old::transit::LOAD_KIND)
        .filter_map(|option| {
            let text = |key: &str| option.payload.get(key).and_then(serde_json::Value::as_str);
            let flag = |key: &str| option.payload.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
            Some(Pool { system: text("pickup_system")?.to_owned(), planet: text("source").map(str::to_owned), unit: text("unit")?.to_owned(), damaged: flag("damaged"), galvanized: flag("galvanized") })
        })
        .collect()
}

/// Every destination worth asking: the systems some ship of `player` can reach, then a spread of the rest.
fn destinations(state: &GameState, galaxy: &Galaxy, sources: SourceSet, player: &PlayerId, cap: usize) -> Vec<SystemId> {
    let content = content();
    let all: Vec<SystemId> = galaxy.system_ids().into_iter().map(SystemId::new).collect();
    let mut reached = Vec::new();
    let mut rest = Vec::new();
    for dest in &all {
        if reached.len() >= cap {
            break;
        }
        if thin::queries::tactical::movable_into(state, content, sources, galaxy, player, dest).is_empty() {
            rest.push(dest.clone());
        } else {
            reached.push(dest.clone());
        }
    }
    let stride = (rest.len() / 4).max(1);
    reached.extend(rest.into_iter().step_by(stride).take(4));
    reached
}

fn compare(label: &str, state: &GameState, galaxy: &Galaxy, sources: SourceSet, who: &[PlayerId], cap: usize, counts: &mut Counts) {
    let content = content();
    let types = catalogue(content, sources);
    for player in who {
        for active in destinations(state, galaxy, sources, player, cap) {
            let mut st = state.clone();
            st.active_system = Some(active.clone());
            let st = &st;
            let mut rules = old::movement::MovementRules::with_laws(galaxy, content, sources, active.as_str(), old::movement::Board::for_player(st, content, sources, player), Some(st));
            old::action_cards::apply_movement_effects(&mut rules, st, player);
            let view = MoveView::new(st, sources, galaxy, player, &active).unwrap_or_else(|| panic!("{label}: the core holds the position ({player} -> {active})"));
            counts.views += 1;
            let ctx = format!("{label}: {player} -> {active}");
            let rolls = !rules.anomalies_ignored && !rules.rifts_ignored;
            assert_eq!(rolls, view.rolls(), "{ctx}: rolls");
            counts.unrolled += usize::from(!rolls);
            for origin in st.systems_with_units_of(player) {
                if *origin == active {
                    continue;
                }
                assert_eq!(rules.may_depart(origin.as_str()), view.may_depart(origin.as_str()), "{ctx}: may_depart {origin}");
                assert_eq!(rules.nebula_caps(origin.as_str()), view.nebula_caps(origin.as_str()), "{ctx}: nebula_caps {origin}");
                counts.departures_blocked += usize::from(!rules.may_depart(origin.as_str()));
                for (index, hull) in st.ships_of(player, origin).into_iter().enumerate() {
                    let Some(kind) = types.get(hull.type_id.as_str()) else { continue };
                    if !kind.moves_as_ship() {
                        continue;
                    }
                    counts.ships += 1;
                    for boosts in [(false, false), (true, false), (false, true), (true, true)] {
                        let value = thin::queries::tactical::effective_move_value_for_ship(st, kind, player, origin, Some(index), boosts.0, boosts.1);
                        let ty = hull.type_id.as_str();
                        let want = rules.path_from_ship(origin.as_str(), value, Some(ty));
                        let got = view.path_from_ship(origin.as_str(), value, Some(ty));
                        let ctx = format!("{ctx}: {origin}#{index} {ty} {boosts:?} move {value}");
                        assert_eq!(want, got, "{ctx}: path");
                        let Some(path) = want else { continue };
                        counts.paths += 1;
                        let (o, t) = (old::transit::rifts_exited(&rules, &path), view.rifts_exited(&path));
                        assert_eq!(o, t, "{ctx}: rifts_exited {path:?}");
                        counts.rifts += o.len();
                        let hold = old::transit::CargoWindow::for_ship(st, content, sources, player, origin, hull, &path);
                        let (o, t) = (pool_key(hold.pending_choice()), view.pools(origin.as_str(), ty, &path));
                        assert_eq!(o, t, "{ctx}: pools {path:?}");
                        counts.pools += o.len();
                        let o = old::tactical::ship_routes(st, content, sources, galaxy, &rules, player, origin, index, hull, boosts, "");
                        let t = view.routes(origin.as_str(), index, ty, boosts, "");
                        assert_eq!(serde_json::to_string(&o).unwrap(), serde_json::to_string(&t).unwrap(), "{ctx}: routes");
                        counts.routes += o.len();
                        counts.multi_routes += usize::from(o.len() > 1);
                        for route in &o {
                            let hold = old::transit::CargoWindow::for_ship(st, content, sources, player, origin, hull, &route.path);
                            assert_eq!(pool_key(hold.pending_choice()), view.pools(origin.as_str(), ty, &route.path), "{ctx}: pools on route {:?}", route.path);
                        }
                    }
                }
            }
        }
    }
}

fn players(state: &GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

#[test]
fn the_movement_facts_of_played_games_answer_as_the_string_helpers_do() {
    let mut counts = Counts::default();
    for pos in positions() {
        let who = players(&pos.state);
        compare(&pos.label, &pos.state, &pos.galaxy, pos.sources, &who, 6, &mut counts);
    }
    println!("{counts:?}");
    assert!(counts.ships > 100 && counts.paths > 100, "{counts:?}");
    assert!(counts.pools > 20, "{counts:?}");
    assert!(counts.departures_blocked > 5, "{counts:?}");
}

/// A fleet of the first seat stood among everything the movement rules read: enemy ships, command tokens, a wormhole token, a law.
fn crowd(state: &mut GameState, who: &[PlayerId]) {
    let unit = |kind: &str, owner: &PlayerId| Unit::new(UnitTypeId::new(kind), owner.clone());
    let ids: Vec<SystemId> = state.board.keys().cloned().collect();
    for (i, id) in ids.iter().enumerate() {
        let cell = state.board.get_mut(id).expect("a cell");
        match i % 5 {
            0 => cell.units.push(unit("carrier", &who[0])),
            1 => cell.units.push(unit("cruiser", &who[1 % who.len()])),
            2 => cell.command_tokens.insert(who[0].clone()).then_some(()).unwrap_or(()),
            3 => {
                cell.units.push(unit("destroyer", &who[0]));
                cell.units.push(unit("fighter", &who[0]));
                cell.units.push(unit("fighter", &who[0]));
            }
            _ => {}
        }
        if i % 7 == 0 && let Some(planet) = cell.planet_control.keys().next().cloned() {
            cell.planet_units.entry(planet).or_default().push(unit("infantry", &who[0]));
        }
    }
}

type Mutation = (&'static str, fn(&mut GameState, &[PlayerId]));

const MUTATIONS: [Mutation; 9] = [
    ("crowd", crowd),
    ("shared research", |s, _| {
        s.laws.insert("shared_research".to_owned(), String::new());
    }),
    ("nav suite this activation", |s, who| {
        let seq = s.activation_seq;
        s.player_mut(&who[0]).expect("seat").anomalies_ignored_activation = Some(seq);
    }),
    ("dominus orb", |s, who| {
        let seq = s.activation_seq;
        s.player_mut(&who[0]).expect("seat").dominus_orb.push(seq);
    }),
    ("light wave deflector", |s, who| {
        s.player_mut(&who[0]).expect("seat").technologies.insert(ti4_model::id::TechnologyId::new("lwd"));
    }),
    ("circlet of the void", |s, who| {
        s.player_mut(&who[0]).expect("seat").relics.push(ti4_model::id::RelicId::new("circletofthevoid"));
    }),
    ("silence", |s, who| {
        let (seq, at) = (s.activation_seq, s.board.keys().nth(3).cloned());
        let seat = s.player_mut(&who[0]).expect("seat");
        seat.silence_activation = Some(seq);
        seat.silence_system = at;
    }),
    ("gamma wormhole token", |s, _| {
        let mut systems = s.board.keys().cloned();
        let (a, b) = (systems.nth(2), systems.nth(5));
        if let (Some(a), Some(_)) = (a, b) {
            s.wormhole_tokens.insert("GAMMA".to_owned(), a);
        }
    }),
    ("every upgrade and tech", |s, who| {
        let seat = s.player_mut(&who[0]).expect("seat");
        for tech in ["gd", "lwd", "aida", "ie", "nm", "ps", "sar", "st"] {
            seat.technologies.insert(ti4_model::id::TechnologyId::new(tech));
        }
    }),
];

#[test]
fn the_movement_facts_follow_the_cards_laws_and_tokens_in_play() {
    let mut counts = Counts::default();
    let content = content();
    let _ = content;
    for (players_n, seed, steps) in [(3, 3, 0), (4, 9, 200), (6, 2, 400), (5, 13, 700)] {
        let pos = surface_common::position(players_n, seed, steps);
        let who = players(&pos.state);
        for (name, mutate) in MUTATIONS {
            let mut state = pos.state.clone();
            mutate(&mut state, &who);
            compare(&format!("{} + {name}", pos.label), &state, &pos.galaxy, pos.sources, &who[..2], 5, &mut counts);
        }
    }
    println!("{counts:?}");
    assert!(counts.unrolled > 0, "no variant ignored anomalies: {counts:?}");
    assert!(counts.multi_routes > 0, "no ship had several routes: {counts:?}");
}

fn template_position(players_n: usize, seed: u64, template: &str) -> Option<Pos> {
    let content = content();
    let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f"][..players_n].iter().map(|n| PlayerId::new(*n)).collect();
    let (state, galaxy) = ti4_host::map::create_game_with_template(content, &ids, seed, Some(template)).ok()?;
    Some(Pos { label: format!("{players_n}p seed {seed} template {template}"), state, galaxy, sources: POK })
}

#[test]
fn the_movement_facts_on_template_maps_with_hyperlanes_answer_as_the_string_helpers_do() {
    let mut counts = Counts::default();
    let mut lanes = 0;
    for players_n in [3, 4, 5, 6] {
        for summary in ti4_host::maps::catalog::catalog(Some(players_n)).expect("the templates load") {
            if !summary.hyperlanes {
                continue;
            }
            let Some(mut pos) = template_position(players_n, 7, &summary.alias) else { continue };
            lanes += 1;
            let who = players(&pos.state);
            crowd(&mut pos.state, &who);
            compare(&pos.label, &pos.state, &pos.galaxy, pos.sources, &who[..2], 6, &mut counts);
        }
    }
    println!("lane maps {lanes} {counts:?}");
    assert!(lanes > 0, "no template with a hyperlane was found");
    assert!(counts.paths > 20, "{counts:?}");
}

/// Ships that stand in, and ships that route through, gravity rifts: random boards often hold none, so seeds are searched.
#[test]
fn the_rifts_a_route_exits_answer_as_the_string_helpers_do() {
    let content = content();
    let index = ti4_content::galaxy::systems_index(content, POK);
    let mut counts = Counts::default();
    let mut boards = 0;
    let templates: Vec<(usize, String)> = [3_usize, 4, 5, 6].into_iter().flat_map(|n| ti4_host::maps::catalog::catalog(Some(n)).expect("the templates load").into_iter().map(move |t| (n, t.alias))).collect();
    for (seed, (n, alias)) in (1_u64..).zip(templates) {
        let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f"][..n].iter().map(|n| PlayerId::new(*n)).collect();
        let Ok((mut state, galaxy)) = ti4_host::map::create_game_with_template(content, &ids, seed, Some(&alias)) else { continue };
        let rifts: Vec<SystemId> = galaxy.system_ids().into_iter().filter(|id| index.get(*id).is_some_and(|s| s.is_gravity_rift())).map(SystemId::new).collect();
        if rifts.is_empty() {
            continue;
        }
        boards += 1;
        let who = players(&state);
        crowd(&mut state, &who);
        for rift in &rifts {
            let cell = state.board.entry(rift.clone()).or_default();
            for kind in ["cruiser", "destroyer", "carrier", "dreadnought"] {
                cell.units.push(Unit::new(UnitTypeId::new(kind), who[0].clone()));
            }
            cell.units.push(Unit::new(UnitTypeId::new("cruiser"), who[1].clone()));
        }
        for (name, mutate) in MUTATIONS.iter().filter(|(n, _)| ["crowd", "circlet of the void", "nav suite this activation", "every upgrade and tech"].contains(n)).chain(std::iter::once(&("plain", (|_: &mut GameState, _: &[PlayerId]| {}) as fn(&mut GameState, &[PlayerId])))) {
            let mut variant = state.clone();
            mutate(&mut variant, &who);
            compare(&format!("seed {seed} rifts {rifts:?} + {name}"), &variant, &galaxy, POK, &who[..2], 10, &mut counts);
        }
        if boards >= 6 {
            break;
        }
    }
    println!("{counts:?}");
    assert!(boards > 0, "no board with a gravity rift");
    assert!(counts.rifts > 0, "no rift was exited: {counts:?}");
    assert!(counts.unrolled > 0, "{counts:?}");
}

/// A three-seat table on a real map whose first seat plays `faction`, with every unit upgrade, every leader unlocked and a crowd.
fn armed_table(faction: &str, seed: u64) -> (GameState, Galaxy) {
    let content = content();
    let ids = [PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c")];
    let others: Vec<&str> = ["sol", "hacan", "letnev"].into_iter().filter(|other| *other != faction).take(2).collect();
    let assignments: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId> = [("a", faction), ("b", others[0]), ("c", others[1])].into_iter().map(|(p, f)| (PlayerId::new(p), ti4_model::id::FactionId::new(f))).collect();
    let (mut state, galaxy) = (seed..seed + 60)
        .find_map(|seed| ti4_host::map::create_game_with_template_seated(content, &ids, seed, None, &assignments, None).ok())
        .unwrap_or_else(|| panic!("a map with {faction}"));
    let seat = state.player_mut(&ids[0]).expect("a seat");
    for status in seat.leaders.values_mut() {
        *status = ti4_model::state::LeaderStatus::Unlocked;
    }
    for tech in ["gd", "lwd", "aida", "ie", "nm", "ps", "sar", "st"] {
        seat.technologies.insert(ti4_model::id::TechnologyId::new(tech));
    }
    crowd(&mut state, &ids);
    (state, galaxy)
}

#[test]
fn every_lobby_faction_moves_as_the_string_helpers_say() {
    let mut counts = Counts::default();
    for (n, faction) in old::seating::LOBBY_FACTIONS.iter().enumerate() {
        let (state, galaxy) = armed_table(faction, 3 + n as u64);
        let who = players(&state);
        compare(&format!("faction {faction}"), &state, &galaxy, POK, &who, 5, &mut counts);
    }
    println!("{counts:?}");
    assert!(counts.paths > 500, "{counts:?}");
}

/// The Cabal's dimensional tears make rifts anywhere, and their owner does not roll for them (`rift_roll_exempt`).
#[test]
fn dimensional_tears_make_rifts_the_owner_does_not_roll_for() {
    let mut counts = Counts::default();
    for seed in [3_u64, 11, 19] {
        let (mut state, galaxy) = armed_table("cabal", seed);
        let who = players(&state);
        let ids: Vec<SystemId> = galaxy.system_ids().into_iter().map(SystemId::new).collect();
        // A dock of the Cabal seat in a few systems on the way: tears, rifts for all.
        for id in ids.iter().skip(1).step_by(2).take(20) {
            let cell = state.board.entry(id.clone()).or_default();
            cell.units.push(Unit::new(UnitTypeId::new("cabal_spacedock"), who[0].clone()));
        }
        compare(&format!("tears seed {seed}"), &state, &galaxy, POK, &who, 8, &mut counts);
    }
    println!("{counts:?}");
    assert!(counts.rifts > 0, "{counts:?}");
}
