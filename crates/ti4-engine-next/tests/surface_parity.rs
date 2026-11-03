//! Lane B (docs/todo/D2-8-wrapper.md, 8d): every native item of the product surface answers as its legacy original does, on the same inputs.
//! Constants are compared one by one; queries over positions played forward on real maps (`surface_common`), for every seat and system.

mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::id::{PlayerId, SystemId};

#[test]
fn the_kinds_are_the_legacy_constants() {
    use thin::kinds as k;
    assert_eq!(k::action_cards::DISCARD_KIND, old::action_cards::DISCARD_KIND);
    assert_eq!(k::action_cards::TAKE_REVEALED_KIND, old::action_cards::TAKE_REVEALED_KIND);
    assert_eq!(k::action_cards::OWN_HAND_KIND, old::action_cards::OWN_HAND_KIND);
    assert_eq!(k::action_cards::ACTION_CARD_KIND, old::reactions::ACTION_CARD_KIND);
    assert_eq!(k::combat::SUSTAIN_KIND, old::combat::SUSTAIN_KIND);
    assert_eq!(k::combat::CASUALTY_KIND, old::combat::CASUALTY_KIND);
    assert_eq!(k::combat::RETREAT_KIND, old::combat::RETREAT_KIND);
    assert_eq!(k::combat::RETREAT_TO_KIND, old::combat::RETREAT_TO_KIND);
    assert_eq!(k::diplomacy::OPEN_KIND, old::diplomacy::candidates::OPEN_KIND);
    assert_eq!(k::draft::STRATEGY_CARD_KIND, old::draft::STRATEGY_CARD_KIND);
    assert_eq!(k::game::CONTINUE_ACTION_ID, old::game::CONTINUE_ACTION_ID);
    assert_eq!(k::game::CONTINUE_ACTION_KIND, old::game::CONTINUE_ACTION_KIND);
    assert_eq!(k::game::SECONDARY_PREVIEW_COMPLETE, old::game::SECONDARY_PREVIEW_COMPLETE);
    assert_eq!(k::game::END_TURN_ID, old::game::END_TURN_ID);
    assert_eq!(k::game::END_TURN_KIND, old::game::END_TURN_KIND);
    assert_eq!(k::game::TACTICAL_ACTION_ID, old::game::TACTICAL_ACTION_ID);
    assert_eq!(k::invasion::COMMIT_KIND, old::invasion::COMMIT_KIND);
    assert_eq!(k::invasion::GROUND_CASUALTY_KIND, old::invasion::GROUND_CASUALTY_KIND);
    assert_eq!(k::neutral_units::NEUTRAL, old::neutral_units::NEUTRAL);
    assert_eq!(k::objectives::VICTORY_TARGET, old::objectives::VICTORY_TARGET);
    assert_eq!(k::objectives::SCORE_KIND, old::objectives::SCORE_KIND);
    assert_eq!(k::production::PRODUCE_KIND, old::production::PRODUCE_KIND);
    assert_eq!(k::production::PAY_KIND, old::production::PAY_KIND);
    assert_eq!(k::production::PLACE_KIND, old::production::PLACE_KIND);
    assert_eq!(k::production::SPACE, old::production::SPACE);
    assert_eq!(k::production::REMOTE_SEPARATOR, old::production::REMOTE_SEPARATOR);
    assert_eq!(k::production::AGENT_OPTION_ID, old::production::AGENT_OPTION_ID);
    assert_eq!(k::production::AGENT_OPTION_KIND, old::production::AGENT_OPTION_KIND);
    assert_eq!(k::production::BUILDABLE, old::production::BUILDABLE);
    assert_eq!(k::production::UNLOCKED_BY, old::production::UNLOCKED_BY);
    assert_eq!(k::promissory::SUPPORT_PREFIX, old::promissory::SUPPORT_PREFIX);
    assert_eq!(k::relics::SHARD, old::relics::SHARD);
    assert_eq!(k::secondary_preview::MAX_ANSWERS, old::secondary_preview::MAX_ANSWERS);
    assert_eq!(k::secrets::HAND_LIMIT, old::secrets::HAND_LIMIT);
    assert_eq!(k::seating::MECATOL, old::seating::MECATOL);
    assert_eq!(k::seating::MECATOL_TE, old::seating::MECATOL_TE);
    assert_eq!(k::starting_tech::KIND, old::starting_tech::KIND);
    assert_eq!(k::strategy::WAIVED_SECONDARY_PREFIX, old::strategy::WAIVED_SECONDARY_PREFIX);
    assert_eq!(k::strategy::STRATEGY_KIND, old::strategy::STRATEGY_KIND);
    assert_eq!(k::strategy::ACTION_KIND, old::strategy::ACTION_KIND);
    assert_eq!(k::strategy::STRATEGIC_ACTION_ID, old::strategy::STRATEGIC_ACTION_ID);
    assert_eq!(k::strategy_cards::INFLUENCE_PER_TOKEN, old::strategy_cards::INFLUENCE_PER_TOKEN);
    assert_eq!(k::strategy_cards::RESEARCH_KIND, old::strategy_cards::RESEARCH_KIND);
    assert_eq!(k::tactical::ROUTES_DETAIL, old::tactical::ROUTES_DETAIL);
    assert_eq!(k::tactical::ACTIVATE_KIND, old::tactical::ACTIVATE_KIND);
    assert_eq!(k::tactical::MOVE_KIND, old::tactical::MOVE_KIND);
    assert_eq!(k::tactical::MAX_ROUTES, old::tactical::MAX_ROUTES);
    assert_eq!(k::timing::ACTIVE_DETAIL, old::timing::ACTIVE_DETAIL);
    assert_eq!(k::transactions::ANSWER_KIND, old::transactions::ANSWER_KIND);
    assert_eq!(k::transactions::FREE_FORM_PREFIX, old::transactions::FREE_FORM_PREFIX);
    assert_eq!(k::transactions::OFFER_KIND, old::transactions::OFFER_KIND);
    assert_eq!(k::transactions::OPEN_KIND, old::transactions::OPEN_KIND);
    assert_eq!(k::transit::LOAD_KIND, old::transit::LOAD_KIND);
    assert_eq!(k::transit::RIFT_DESTROYS_ON, old::transit::RIFT_DESTROYS_ON);
}

fn players(state: &ti4_model::state::GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

/// `Movable`/`MoveRoute` do not share a type with the legacy ones: compared through their fields.
fn movable_key(m: &thin::queries::tactical::Movable) -> String {
    format!("{} {} {:?} {} {} {}", m.origin, m.index, m.unit, m.capacity, m.gravity_drive, m.ionian)
}
fn movable_key_old(m: &old::tactical::Movable) -> String {
    format!("{} {} {:?} {} {} {}", m.origin, m.index, m.unit, m.capacity, m.gravity_drive, m.ionian)
}

#[test]
fn the_movement_queries_answer_as_the_legacy_ones_do() {
    let content = content();
    let (mut ships, mut routes) = (0_usize, 0_usize);
    for pos in positions() {
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        for player in players(&pos.state) {
            for dest in &systems {
                let o: Vec<String> = old::tactical::movable_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest).iter().map(movable_key_old).collect();
                let t: Vec<String> = thin::queries::tactical::movable_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest).iter().map(movable_key).collect();
                assert_eq!(o, t, "{}: movable_into {player} -> {dest}", pos.label);
                ships += o.len();
                for (origin, index) in pos.state.systems_with_units_of(&player).into_iter().flat_map(|s| (0..pos.state.ships_of(&player, s).len()).map(move |i| (s.clone(), i))).take(6) {
                    for boosts in [(false, false), (true, false), (false, true), (true, true)] {
                        let o = old::tactical::routes_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest, &origin, index, boosts);
                        let t = thin::queries::tactical::routes_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest, &origin, index, boosts);
                        assert_eq!(serde_json::to_string(&o).unwrap(), serde_json::to_string(&t).unwrap(), "{}: routes_into {player} {origin}#{index} {boosts:?} -> {dest}", pos.label);
                        routes += o.len();
                    }
                }
            }
        }
    }
    assert!(ships > 50, "only {ships} movable ships compared");
    assert!(routes > 50, "only {routes} routes compared");
}

#[test]
fn the_production_queries_answer_as_the_legacy_ones_do() {
    use thin::queries::production as t;
    let content = content();
    let types = ti4_content::units::catalogue(content, ti4_model::content_types::POK);
    let mut compared = 0_usize;
    for pos in positions() {
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        for player in players(&pos.state) {
            for (o, n) in [(old::production::Spend::Resources, t::Spend::Resources), (old::production::Spend::Influence, t::Spend::Influence)] {
                assert_eq!(
                    old::production::available(&pos.state, content, pos.sources, &player, o),
                    t::available(&pos.state, content, pos.sources, &player, n),
                    "{}: available {player} {o:?}",
                    pos.label
                );
            }
            assert_eq!(old::production::trade_good_worth(&pos.state, &player), t::trade_good_worth(&pos.state, &player), "{}: trade_good_worth {player}", pos.label);
            for system in &systems {
                assert_eq!(
                    old::production::capacity(&pos.state, content, pos.sources, &player, system),
                    t::capacity(&pos.state, content, pos.sources, &player, system),
                    "{}: capacity {player} {system}",
                    pos.label
                );
                for unit in old::production::BUILDABLE {
                    let kind = types.get(unit).expect("a buildable unit");
                    assert_eq!(
                        old::production::placements(&pos.state, content, pos.sources, &player, system, kind),
                        t::placements(&pos.state, content, pos.sources, &player, system, kind),
                        "{}: placements {player} {system} {unit}",
                        pos.label
                    );
                    compared += 1;
                }
            }
        }
        let mut with_law = pos.state.clone();
        with_law.laws.insert("conscription".to_owned(), "for".to_owned());
        for state in [Some(&pos.state), Some(&with_law), None] {
            for kind in types.values() {
                assert_eq!(old::production::price_of_under(state, kind), t::price_of_under(state, kind), "{}: price_of_under {}", pos.label, kind.id());
            }
        }
    }
    assert!(compared > 1000, "only {compared} placement questions compared");
}

/// The accessors of `Observed` that ask a rule module, on positions with a map: the native view against the legacy one.
#[test]
fn the_observed_accessors_that_ask_rule_modules_answer_as_the_legacy_view_does() {
    use ti4_model::content_types::POK;
    let content = content();
    let mut compared = 0_usize;
    for pos in positions() {
        let (o, t) = (
            old::choice::Observed::new(&pos.state, content, POK, Some(&pos.galaxy)),
            thin::observed::Observed::new(&pos.state, content, POK, Some(&pos.galaxy)),
        );
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        let at = |what: &str| format!("{}: {what}", pos.label);
        for system in &systems {
            assert_eq!(o.is_fracture_system(system), t.is_fracture_system(system), "{}", at("is_fracture_system"));
            assert_eq!(o.active_breach_in(system), t.active_breach_in(system), "{}", at("active_breach_in"));
        }
        for player in players(&pos.state) {
            let who = |what: &str| format!("{}: {player}: {what}", pos.label);
            assert_eq!(o.fleet_supply_limit(&player), t.fleet_supply_limit(&player), "{}", who("fleet_supply_limit"));
            assert_eq!(o.mahact_fleet_pool_owners(&player), t.mahact_fleet_pool_owners(&player), "{}", who("mahact_fleet_pool_owners"));
            assert_eq!(o.firmament_puppets(&player), t.firmament_puppets(&player), "{}", who("firmament_puppets"));
            assert_eq!(o.non_home_systems_with_planets(&player), t.non_home_systems_with_planets(&player), "{}", who("non_home_systems_with_planets"));
            assert_eq!(o.opening_fleet(&player), t.opening_fleet(&player), "{}", who("opening_fleet"));
            assert_eq!(o.has_units_in_fracture(&player), t.has_units_in_fracture(&player), "{}", who("has_units_in_fracture"));
            assert_eq!(o.seat(&player).map(|s| s.fleet_tokens), t.seat(&player).map(|s| s.fleet_tokens), "{}", who("seat.fleet_tokens"));
            assert_eq!(
                o.available_spend(&player, old::production::Spend::Influence),
                t.available_spend(&player, thin::queries::production::Spend::Influence),
                "{}",
                who("available_spend")
            );
            for other in players(&pos.state) {
                assert_eq!(o.recent_diplomacy_attack(&player, &other), t.recent_diplomacy_attack(&player, &other), "{}", who("recent_diplomacy_attack"));
                assert_eq!(o.recent_diplomacy_breach(&player, &other), t.recent_diplomacy_breach(&player, &other), "{}", who("recent_diplomacy_breach"));
            }
            for origin in pos.state.systems_with_units_of(&player) {
                assert_eq!(format!("{:?}", o.loadable(&player, origin)), format!("{:?}", t.loadable(&player, origin)), "{}", who("loadable"));
                for dest in &systems {
                    for value in [1, 2, 3] {
                        assert_eq!(o.can_reach(&player, origin, dest, value), t.can_reach(&player, origin, dest, value), "{}", who("can_reach"));
                    }
                    for unit in ["fighter", "carrier", "cruiser", "dreadnought", "warsun", "flagship"] {
                        for boosts in [(false, false), (true, true)] {
                            assert_eq!(o.rift_exits_into(&player, origin, unit, boosts, dest), t.rift_exits_into(&player, origin, unit, boosts, dest), "{}", who("rift_exits_into"));
                            assert_eq!(o.typed_rift_exits(&player, origin, unit, boosts, dest), t.typed_rift_exits(&player, origin, unit, boosts, dest), "{}", who("typed_rift_exits"));
                        }
                    }
                    compared += 1;
                }
            }
            for dest in &systems {
                let (om, tm) = (o.movable_into(&player, dest), t.movable_into(&player, dest));
                assert_eq!(om.iter().map(movable_key_old).collect::<Vec<_>>(), tm.iter().map(movable_key).collect::<Vec<_>>(), "{}", who("movable_into"));
                assert_eq!(o.movable_into_shared(&player, dest).len(), t.movable_into_shared(&player, dest).len(), "{}", who("movable_into_shared"));
                assert_eq!(om.len(), t.typed_movable_into(&player, dest).map_or(0, |v| v.len()), "{}", who("typed_movable_into"));
                for origin in pos.state.systems_with_units_of(&player).into_iter().take(3) {
                    for index in 0..pos.state.ships_of(&player, origin).len().min(3) {
                        assert_eq!(
                            serde_json::to_string(&o.ship_routes_into(&player, origin, index, (false, false), dest)).unwrap(),
                            serde_json::to_string(&t.ship_routes_into(&player, origin, index, (false, false), dest)).unwrap(),
                            "{}",
                            who("ship_routes_into")
                        );
                    }
                }
            }
        }
    }
    assert!(compared > 500, "only {compared} reach questions compared");
}

#[test]
fn the_fleet_limit_of_a_table_the_core_cannot_hold_is_the_pool_capped_by_the_law() {
    let content = content();
    let mut state = old::fixtures::game(&["a", "b", "c"]);
    for tokens in [0, 3, 6, 9] {
        for law in [false, true] {
            state.players[0].fleet_tokens = tokens;
            state.laws.clear();
            if law {
                state.laws.insert("regulations".to_owned(), "for".to_owned());
            }
            let player = state.players[0].id.clone();
            assert_eq!(old::fleet::limit(&state, content, &player), thin::queries::factions::fleet_limit(&state, content, &player), "{tokens} tokens, law {law}");
        }
    }
}

#[test]
fn the_content_reads_answer_as_the_legacy_ones_do() {
    use ti4_model::content_types::{ContentType, FULL, POK};
    use ti4_model::id::{ObjectiveId, PlanetId};
    let content = content();
    let planets: Vec<PlanetId> = ti4_content::galaxy::all_planets(content, FULL).keys().map(|id| PlanetId::new(*id)).collect();
    assert!(planets.len() > 100);
    let cards: Vec<String> = content.records(ContentType::Explores).iter().filter_map(|r| r.id().map(str::to_owned)).collect();
    assert!(cards.len() > 40);
    for sources in [POK, FULL] {
        for planet in &planets {
            assert_eq!(old::exploration::traits_of(content, sources, planet), thin::queries::content::traits_of(content, sources, planet), "traits_of {planet}");
        }
    }
    for card in &cards {
        assert_eq!(old::exploration::resolution(content, card), thin::queries::content::resolution(content, card), "resolution {card}");
        for planet in planets.iter().step_by(7) {
            assert_eq!(old::exploration::attachment_for(content, card, planet), thin::queries::content::attachment_for(content, card, planet), "attachment_for {card} {planet}");
        }
    }
    let objectives: Vec<ObjectiveId> = [ContentType::PublicObjectives, ContentType::SecretObjectives]
        .into_iter()
        .flat_map(|c| content.records(c).iter().filter_map(|r| r.id().map(ObjectiveId::new)).collect::<Vec<_>>())
        .chain([ObjectiveId::new("no-such-objective")])
        .collect();
    assert!(objectives.len() > 40);
    for objective in &objectives {
        assert_eq!(old::objectives::points_for(content, objective), thin::queries::content::points_for(content, objective), "points_for {objective}");
    }
    for pos in positions() {
        let mut state = pos.state.clone();
        for vp in [0, 3, 9, 10, 11] {
            for (i, player) in state.players.iter_mut().enumerate() {
                player.victory_points = if i % 2 == 0 { vp } else { vp / 2 };
            }
            assert_eq!(old::objectives::leader(&state), thin::queries::content::leader(&state), "{}: leader at {vp}", pos.label);
            assert_eq!(old::objectives::winner(&state), thin::queries::content::winner(&state), "{}: winner at {vp}", pos.label);
        }
        for player in players(&pos.state) {
            assert_eq!(old::secrets::scored_count(&pos.state, content, &player), thin::queries::content::scored_count(&pos.state, content, &player), "{}: scored_count", pos.label);
        }
    }
}

#[test]
fn the_fleet_strength_functions_are_the_legacy_ones() {
    use ti4_model::content_types::POK;
    let content = content();
    let fleets: Vec<Vec<(String, usize)>> = vec![
        vec![],
        vec![("fighter".to_owned(), 4)],
        vec![("carrier".to_owned(), 2), ("fighter".to_owned(), 6), ("dreadnought".to_owned(), 1)],
        vec![("cruiser".to_owned(), 3), ("destroyer".to_owned(), 2), ("warsun".to_owned(), 1), ("flagship".to_owned(), 1), ("no-such-unit".to_owned(), 2)],
        vec![("dreadnought".to_owned(), 3), ("pds".to_owned(), 2), ("mech".to_owned(), 2), ("infantry".to_owned(), 5)],
    ];
    let damaged = vec![("dreadnought".to_owned(), 1), ("warsun".to_owned(), 1)];
    for faction in ["sol", "jolnar", "sardakk", "letnev", "", "no-such-faction"] {
        assert_eq!(old::fleet_strength::modifier(faction), thin::queries::fleet_strength::modifier(faction), "{faction}");
        let mut seen = Vec::new();
        for fleet in &fleets {
            for dmg in [&Vec::new(), &damaged] {
                let o = old::fleet_strength::descriptors(content, POK, faction, fleet, dmg);
                let t = thin::queries::fleet_strength::descriptors(content, POK, faction, fleet, dmg);
                assert_eq!(format!("{o:?}"), format!("{t:?}"), "descriptors {faction} {fleet:?}");
                seen.push((o, t));
            }
        }
        for (o1, t1) in &seen {
            for (o2, t2) in &seen {
                assert_eq!(old::fleet_strength::matchup(o1, o2).to_bits(), thin::queries::fleet_strength::matchup(t1, t2).to_bits(), "matchup");
            }
        }
    }
    for hits_on in 0..13 {
        for modifier in -3..4 {
            assert_eq!(old::fleet_strength::hit_chance(hits_on, modifier).to_bits(), thin::queries::fleet_strength::hit_chance(hits_on, modifier).to_bits());
        }
    }
}

#[test]
fn the_technology_reads_answer_as_the_legacy_ones_do() {
    use ti4_model::content_types::{ContentType, FULL, POK};
    use ti4_model::id::TechnologyId;
    let content = content();
    let aliases: Vec<TechnologyId> = content.records(ContentType::Technologies).iter().filter_map(|r| r.text("alias").map(TechnologyId::new)).chain([TechnologyId::new("no-such-tech")]).collect();
    assert!(aliases.len() > 50);
    for sources in [POK, FULL] {
        assert_eq!(old::technology::active_aliases(content, sources), thin::queries::technology::active_aliases(content, sources));
    }
    let factions: Vec<String> = content.records(ContentType::Factions).iter().filter_map(|r| r.id().map(str::to_owned)).chain([String::new(), "no-such-faction".to_owned()]).collect();
    for alias in &aliases {
        assert_eq!(old::technology::name(content, alias), thin::queries::technology::name(content, alias), "name {alias}");
        assert_eq!(old::technology::colour_type(content, alias), thin::queries::technology::colour_type(content, alias), "colour_type {alias}");
        assert_eq!(old::technology::prerequisites(content, alias), thin::queries::technology::prerequisites(content, alias), "prerequisites {alias}");
        assert_eq!(old::technology::is_unit_upgrade(content, alias), thin::queries::technology::is_unit_upgrade(content, alias), "is_unit_upgrade {alias}");
        assert_eq!(old::technology::faction_of(content, alias), thin::queries::technology::faction_of(content, alias), "faction_of {alias}");
        for faction in &factions {
            assert_eq!(old::technology::replaced_for_faction(content, faction, alias), thin::queries::technology::replaced_for_faction(content, faction, alias), "replaced_for_faction {faction} {alias}");
        }
    }
    for pos in positions() {
        let mut state = pos.state.clone();
        for player in players(&pos.state) {
            assert_eq!(old::technology::gravity_drive_available(&state, &player), thin::queries::technology::gravity_drive_available(&state, &player));
            let activation = state.activation_seq;
            let seat = state.player_mut(&player).unwrap();
            seat.technologies.insert(TechnologyId::new("gd"));
            seat.gravity_drive_used_activation = Some(activation);
            assert_eq!(old::technology::gravity_drive_available(&state, &player), thin::queries::technology::gravity_drive_available(&state, &player));
            state.player_mut(&player).unwrap().gravity_drive_used_activation = None;
            assert_eq!(old::technology::gravity_drive_available(&state, &player), thin::queries::technology::gravity_drive_available(&state, &player));
        }
    }
}

/// Regression tests for mutants the position corpus did not kill (dropped-faction marks, a card effect, the diplomacy window, the printed Armada).
#[test]
fn the_crimson_breach_and_diplomacy_window_reads_follow_the_marks() {
    let mut state = old::fixtures::game(&["a", "b"]);
    let (a, b) = (state.players[0].id.clone(), state.players[1].id.clone());
    state.breach_tokens = ["25", "26", "27"].into_iter().map(SystemId::new).collect();
    state.faction_marks.insert("crimson:breach:active:26".to_owned(), "1".to_owned());
    assert_eq!(old::factions::crimson::active_breaches(&state), thin::queries::factions::crimson_active_breaches(&state));
    assert_eq!(thin::queries::factions::crimson_active_breaches(&state), vec![SystemId::new("26")]);
    state.round = 5;
    for last in [3, 4, 5, 6] {
        state.diplomacy.last_attacks.entry(a.clone()).or_default().insert(b.clone(), last);
        state.diplomacy.last_breaches.entry(b.clone()).or_default().insert(a.clone(), last);
        for (x, y) in [(&a, &b), (&b, &a)] {
            assert_eq!(old::diplomacy::recent_attack(&state, x, y), thin::queries::diplomacy::recent_attack(&state, x, y), "attack, event round {last}");
            assert_eq!(old::diplomacy::recent_breach(&state, x, y), thin::queries::diplomacy::recent_breach(&state, x, y), "breach, event round {last}");
        }
    }
    assert!(thin::queries::diplomacy::recent_attack(&state, &a, &b), "an attack of the last round is recent");
}

#[test]
fn the_printed_armada_lifts_the_fleet_limit_of_a_table_the_core_cannot_hold() {
    let content = content();
    let mut state = old::fixtures::game(&["a", "b"]);
    let player = state.players[0].id.clone();
    for faction in ["letnev", "sol"] {
        state.players[0].faction = ti4_model::id::FactionId::new(faction);
        state.players[0].fleet_tokens = 3;
        assert_eq!(old::fleet::limit(&state, content, &player), thin::queries::factions::fleet_limit(&state, content, &player), "{faction}");
    }
    state.players[0].faction = ti4_model::id::FactionId::new("letnev");
    assert_eq!(thin::queries::factions::fleet_limit(&state, content, &player), 5);
}

#[test]
fn can_reach_does_not_count_the_cards_in_play() {
    use ti4_model::content_types::POK;
    let content = content();
    let mut compared = 0_usize;
    for pos in positions().into_iter().take(6) {
        let mut state = pos.state.clone();
        let activation = state.activation_seq;
        for seat in &mut state.players {
            seat.move_bonus_activation = Some(activation);
            seat.anomalies_ignored_activation = Some(activation);
            seat.technologies.insert(ti4_model::id::TechnologyId::new("lwd"));
        }
        let (o, t) = (old::choice::Observed::new(&state, content, POK, Some(&pos.galaxy)), thin::observed::Observed::new(&state, content, POK, Some(&pos.galaxy)));
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        for player in players(&state) {
            for origin in state.systems_with_units_of(&player) {
                for dest in &systems {
                    for value in [1, 2, 3] {
                        assert_eq!(o.can_reach(&player, origin, dest, value), t.can_reach(&player, origin, dest, value), "{}: can_reach with Flank Speed in play", pos.label);
                        compared += 1;
                    }
                }
            }
        }
        // ... while the ships that could move do count it.
        for player in players(&state) {
            for dest in systems.iter().step_by(3) {
                assert_eq!(o.movable_into(&player, dest).iter().map(movable_key_old).collect::<Vec<_>>(), t.movable_into(&player, dest).iter().map(movable_key).collect::<Vec<_>>(), "{}: movable_into with Flank Speed", pos.label);
            }
        }
    }
    assert!(compared > 500);
}
