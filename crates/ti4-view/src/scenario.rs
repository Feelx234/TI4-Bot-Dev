//! Prepared games for testing a client: a seeded game, then a board that a real game only has
//! after hours of play.
//!
//! A scenario is not a saved state. It is a function over the state of a new game, so a game
//! that starts from one is played again from its seed and its answers like any other.

use serde::Serialize;
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::{phase, technology};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{
    LeaderId, PlanetId, PlayerId, RelicId, StrategyCardId, SystemId, TechnologyId, UnitTypeId,
};
use ti4_model::state::{GameState, LeaderStatus, Phase};
use ti4_model::units::Unit;

/// A prepared game that a host can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Scenario {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The game that the scenario is made for: its board is placed on the map of this game.
    pub seed: u64,
    pub players: usize,
}

pub const LATE_GAME: &str = "late-game";
pub const LATE_GAME_DEFLECTORS: &str = "late-game-deflectors";

pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        id: LATE_GAME,
        title: "Late game",
        description: "Round 4, seat A (Sol) has the turn. Ten fleets with every ship type and \
            every ship upgrade, Gravity Drive, cargo on planets and in space on the way. Around \
            them: a gravity rift, a nebula, a supernova, an asteroid field, a wormhole pair, \
            fleets of other seats and a system with Sol's command token. Asteroid fields and \
            the ships of other seats block.",
        seed: 42,
        players: 8,
    },
    Scenario {
        id: LATE_GAME_DEFLECTORS,
        title: "Late game with deflectors",
        description: "The late game, and seat A also has Light/Wave Deflector and Antimass \
            Deflectors: its ships move through the ships of other seats. The engine does not \
            open asteroid fields for Antimass Deflectors yet.",
        seed: 42,
        players: 8,
    },
];

/// The scenario with this id.
#[must_use]
pub fn find(id: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|scenario| scenario.id == id)
}

/// Turns the state of a new game into the scenario `id`. The game must be the one that the
/// scenario names: its seats and the map of its seed.
pub fn apply(
    id: &str,
    state: &mut GameState,
    galaxy: &mut Galaxy,
    content: &ContentStore,
    sources: SourceSet,
) -> Result<(), String> {
    let scenario = find(id).ok_or_else(|| {
        let known: Vec<&str> = SCENARIOS.iter().map(|scenario| scenario.id).collect();
        format!("no scenario '{id}': the scenarios are {}", known.join(", "))
    })?;
    if state.players.len() != scenario.players {
        return Err(format!(
            "the scenario '{id}' is made for {} seats, the game has {}",
            scenario.players,
            state.players.len()
        ));
    }
    late_game(state, galaxy, content, sources, id == LATE_GAME_DEFLECTORS)
}

/// The systems of the late game, on the map of eight seats. Seat A (Sol) is at home in `HOME`.
mod at {
    pub const HOME: &str = "01";
    pub const RIFT: &str = "41";
    pub const NEBULA: &str = "42";
    pub const SUPERNOVA: &str = "43";
    /// Two planets and a space dock, between the home, the nebula and the middle.
    pub const DOCK: &str = "72";
    /// No ships of seat A, only cargo: every path from the home to the middle crosses it.
    pub const DEPOT: &str = "74";
    /// Behind the rift and behind the home of seat H. A beta wormhole is next to it.
    pub const NORTH: &str = "63";
    /// Seat A activated it: nothing leaves.
    pub const TOKEN: &str = "70";
    pub const WEST: &str = "62";
    pub const SOUTH: &str = "24";
    /// Replaced by `ASTEROIDS`: the map of eight seats has no asteroid field.
    pub const REPLACED: &str = "31";
    pub const ASTEROIDS: &str = "44";
    /// Two planets and no units, next to `NORTH`: its carrier lands there without a fight.
    pub const FREE: &str = "38";
    /// The other beta wormhole, on the far side of the map.
    pub const FAR: &str = "25";
    /// Ships of seat C, with ground forces and a PDS.
    pub const ENEMY_C: &str = "32";
    /// Ships of seat B, three planets.
    pub const ENEMY_B: &str = "75";
    /// Every other seat has a command token here.
    pub const CROWDED: &str = "60";
    pub const ALL: [&str; 15] = [
        HOME, RIFT, NEBULA, SUPERNOVA, DOCK, DEPOT, NORTH, TOKEN, WEST, SOUTH, REPLACED, FAR,
        ENEMY_C, ENEMY_B, FREE,
    ];
}

/// Units of one seat in one system: `(base type, count)` in the space area and on planets.
struct Placement<'a> {
    system: &'a str,
    space: &'a [(&'a str, usize)],
    planets: &'a [(&'a str, &'a [(&'a str, usize)])],
}

fn late_game(
    state: &mut GameState,
    galaxy: &mut Galaxy,
    content: &ContentStore,
    sources: SourceSet,
    deflectors: bool,
) -> Result<(), String> {
    for system in at::ALL.into_iter().chain([at::CROWDED]) {
        if galaxy.coord_of(system).is_none() {
            return Err(format!(
                "the late game needs the system {system}, which is not on this map"
            ));
        }
    }
    let seats: Vec<PlayerId> = state.players.iter().map(|seat| seat.id.clone()).collect();
    let [a, b, c] = [&seats[0], &seats[1], &seats[2]];

    // An asteroid field next to the home of seat A.
    galaxy
        .replace_system(content, at::REPLACED, at::ASTEROIDS, sources)
        .map_err(|error| error.to_string())?;
    state.board.remove(&SystemId::new(at::REPLACED));
    state.system_mut(&SystemId::new(at::ASTEROIDS));

    // The action phase of round 4: every seat has a card, and seat A is first.
    let mut cards = std::mem::take(&mut state.unclaimed_strategy_cards);
    cards.sort_by_key(|card| card.as_str() != "leadership");
    let mut cards = cards.into_iter();
    for seat in &seats {
        let card: StrategyCardId = cards
            .next()
            .ok_or_else(|| format!("no strategy card is left for seat {seat}"))?;
        state.deal_strategy_card(seat, card);
    }
    state.unclaimed_strategy_cards = cards.collect();
    state.round = 4;
    if state.phase == Phase::Strategy {
        phase::advance_phase(state);
    }
    if state.active.as_ref() != Some(a) {
        phase::begin_action_turn(state, a);
    }

    let mut technologies = vec!["gd", "cr2", "dd2", "dn2", "ff2", "ac2", "ws"];
    if deflectors {
        technologies.extend(["amd", "lwd"]);
    }
    for technology in technologies {
        technology::grant(state, a, &TechnologyId::new(technology));
    }
    let sol = state.player_mut(a).ok_or("seat A is not seated")?;
    // 16 command tokens: 15 on the sheet and one in `TOKEN`.
    sol.tactic_tokens = 7;
    sol.fleet_tokens = 5;
    sol.strategic_tokens = 3;
    sol.trade_goods = 6;

    // The start units of seat A go: the fleets below are all that it has.
    for system in state.board.values_mut() {
        system.units.retain(|unit| unit.owner != *a);
        for units in system.planet_units.values_mut() {
            units.retain(|unit| unit.owner != *a);
        }
    }
    let fleets_of_a = [
        Placement {
            system: at::HOME,
            space: &[
                ("flagship", 1),
                ("warsun", 1),
                ("carrier", 1),
                ("fighter", 3),
            ],
            planets: &[(
                "jord",
                &[("infantry", 3), ("mech", 1), ("spacedock", 1), ("pds", 1)],
            )],
        },
        Placement {
            system: at::DOCK,
            space: &[
                ("carrier", 1),
                ("cruiser", 1),
                ("destroyer", 1),
                ("fighter", 2),
                ("infantry", 1),
            ],
            planets: &[
                ("lisis", &[("infantry", 2), ("spacedock", 1)]),
                ("velnor", &[("mech", 1), ("infantry", 1)]),
            ],
        },
        Placement {
            system: at::NEBULA,
            space: &[("dreadnought", 1), ("destroyer", 1)],
            planets: &[],
        },
        Placement {
            system: at::RIFT,
            space: &[
                ("carrier", 1),
                ("cruiser", 1),
                ("infantry", 2),
                ("fighter", 2),
            ],
            planets: &[],
        },
        Placement {
            system: at::NORTH,
            space: &[("dreadnought", 1), ("carrier", 1), ("fighter", 1)],
            planets: &[("vorhal", &[("infantry", 2), ("mech", 1)])],
        },
        Placement {
            system: at::FAR,
            space: &[("cruiser", 1), ("destroyer", 1)],
            planets: &[("quann", &[("infantry", 1)])],
        },
        Placement {
            system: at::TOKEN,
            space: &[("cruiser", 1), ("dreadnought", 1)],
            planets: &[("kraag", &[("infantry", 1)])],
        },
        Placement {
            system: at::WEST,
            space: &[("cruiser", 1), ("destroyer", 1)],
            planets: &[("semlore", &[("infantry", 1)])],
        },
        Placement {
            system: at::SOUTH,
            space: &[("dreadnought", 1)],
            planets: &[("meharxull", &[("infantry", 1)])],
        },
        Placement {
            system: at::DEPOT,
            space: &[("fighter", 2)],
            planets: &[
                ("vegamajor", &[("infantry", 2)]),
                ("vegaminor", &[("mech", 1)]),
            ],
        },
    ];
    for fleet in &fleets_of_a {
        place(state, content, sources, a, fleet)?;
    }
    state
        .system_mut(&SystemId::new(at::TOKEN))
        .place_token(a.clone());

    // The other seats, for the paths that they block and for the steps after the movement.
    place(
        state,
        content,
        sources,
        c,
        &Placement {
            system: at::ENEMY_C,
            space: &[("cruiser", 2), ("destroyer", 1)],
            planets: &[("dalbootha", &[("infantry", 2), ("pds", 1)])],
        },
    )?;
    place(
        state,
        content,
        sources,
        b,
        &Placement {
            system: at::ENEMY_B,
            space: &[("carrier", 1), ("cruiser", 1), ("fighter", 2)],
            planets: &[("loki", &[("infantry", 2)])],
        },
    )?;
    for seat in &seats[1..] {
        state
            .system_mut(&SystemId::new(at::CROWDED))
            .place_token(seat.clone());
    }
    for seat in [b, &seats[6], &seats[7]] {
        state
            .system_mut(&SystemId::new(at::ENEMY_C))
            .place_token(seat.clone());
    }
    state
        .system_mut(&SystemId::new(at::DEPOT))
        .place_token(seats[7].clone());

    // What the exploration left: attachments, relic fragments, and the cards out of the decks.
    let drawn: [(&PlayerId, &str, &str, Option<&str>); 9] = [
        (a, "CULTURAL", "ds", Some("vegamajor")),
        (a, "INDUSTRIAL", "cybernetic", Some("velnor")),
        (a, "INDUSTRIAL", "biotic", Some("lisis")),
        (a, "CULTURAL", "crf1", Some("vorhal")),
        (a, "CULTURAL", "crf2", Some("semlore")),
        (a, "CULTURAL", "crf3", Some("vegaminor")),
        (a, "HAZARDOUS", "hrf1", Some("meharxull")),
        (b, "CULTURAL", "pw", Some("loki")),
        (b, "FRONTIER", "urf1", None),
    ];
    for (seat, deck, card, planet) in drawn {
        let cards = state
            .exploration_decks
            .get_mut(deck)
            .ok_or_else(|| format!("the game has no {deck} deck"))?;
        let at = cards
            .iter()
            .position(|held| held == card)
            .ok_or_else(|| format!("the {deck} deck has no card '{card}'"))?;
        cards.remove(at);
        let planet = planet.map(PlanetId::new);
        let outcome = match ti4_engine::exploration::resolution(content, card).as_deref() {
            Some("Attach") => {
                let planet = planet
                    .as_ref()
                    .ok_or_else(|| format!("the card '{card}' needs a planet"))?;
                let attachment = ti4_engine::exploration::attachment_for(content, card, planet);
                state
                    .planet_attachments
                    .entry(planet.clone())
                    .or_default()
                    .push(attachment.clone());
                format!("attached:{attachment}")
            }
            Some("Fragment") => {
                ti4_engine::exploration::gain_fragment(state, seat, deck);
                "fragment".to_owned()
            }
            other => return Err(format!("the card '{card}' resolves as {other:?}")),
        };
        state
            .exploration_log
            .push(ti4_model::state::ExplorationRecord {
                round: 2,
                player: seat.clone(),
                deck: deck.to_owned(),
                card: card.to_owned(),
                planet,
                outcome,
            });
    }

    // The upgrades of seat A are on its sheet: its units on the board are the upgraded ones.
    technology::apply_unit_upgrades(state, content, sources, a);

    // Cards that seat A used this round, and two relics: one ready, one exhausted.
    technology::grant(state, a, &TechnologyId::new("bs"));
    for relic in ["dynamiscore", "prophetstears"] {
        let relic = RelicId::new(relic);
        state.relic_deck.retain(|held| held != &relic);
        state
            .player_mut(a)
            .ok_or("seat A is not seated")?
            .relics
            .push(relic);
    }
    let sol = state.player_mut(a).ok_or("seat A is not seated")?;
    sol.exhausted_technologies.insert(TechnologyId::new("bs"));
    sol.exhausted_relics.insert(RelicId::new("prophetstears"));
    sol.leaders
        .insert(LeaderId::new("solagent"), LeaderStatus::Exhausted);
    Ok(())
}

/// Puts the units of `fleet` on the board. A planet with units of the seat is then its planet.
fn place(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    seat: &PlayerId,
    fleet: &Placement<'_>,
) -> Result<(), String> {
    let faction = state
        .player(seat)
        .ok_or_else(|| format!("seat {seat} is not seated"))?
        .faction
        .to_string();
    let units = |kinds: &[(&str, usize)]| -> Result<Vec<Unit>, String> {
        let mut units = Vec::new();
        for (base, count) in kinds {
            // The unit of the faction where it has its own (Sol's carrier), or the common one.
            let kind = ti4_content::units::faction_unit(content, &faction, base, sources)
                .or_else(|| ti4_content::units::unit_type(content, base, sources))
                .ok_or_else(|| format!("no unit type '{base}'"))?;
            for _ in 0..*count {
                units.push(Unit::new(UnitTypeId::new(kind.id()), seat.clone()));
            }
        }
        Ok(units)
    };
    let known = ti4_content::galaxy::system(content, fleet.system, sources)
        .ok_or_else(|| format!("no system '{}'", fleet.system))?
        .planets();
    let space = units(fleet.space)?;
    let mut landed = Vec::new();
    for (planet, kinds) in fleet.planets {
        if !known.contains(planet) {
            return Err(format!(
                "the system {} has no planet '{planet}'",
                fleet.system
            ));
        }
        landed.push((PlanetId::new(*planet), units(kinds)?));
    }
    let system = state.system_mut(&SystemId::new(fleet.system));
    system.add(&space);
    for (planet, units) in landed {
        system.land(&planet, &units);
        system.set_control(planet, seat.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::{
        Blocked, INVASION_SUBTYPE, MOVEMENT_SUBTYPE, MovementFacts, ShipFact, TacticalFacts,
        project_tactical_facts,
    };
    use ti4_engine::choice::{Choice, ChoiceOption};
    use ti4_engine::decision_context::{DecisionContext, DecisionSource};
    use ti4_model::content_types::POK;

    fn seat() -> PlayerId {
        PlayerId::new("a")
    }

    fn game(id: &str) -> (GameState, Galaxy) {
        let content = ContentStore::embedded();
        let scenario = find(id).expect("a scenario");
        let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f", "g", "h"][..scenario.players]
            .iter()
            .map(|name| PlayerId::new(*name))
            .collect();
        let loader = crate::maps::TemplateLoader::load().expect("the templates load");
        let template = crate::maps::default_template_for(content, &loader, scenario.players, POK)
            .expect("a template");
        let (mut state, mut galaxy) =
            crate::map::create_game_with_template(content, &ids, scenario.seed, Some(&template))
                .expect("a game");
        apply(id, &mut state, &mut galaxy, content, POK).expect("the scenario applies");
        (state, galaxy)
    }

    /// What the movement into `active` shows, in a copy of the game.
    fn facts(state: &GameState, galaxy: &Galaxy, active: &str) -> MovementFacts {
        let mut state = state.clone();
        state.active_system = Some(SystemId::new(active));
        let choice = Choice::new(seat(), "movement", vec![ChoiceOption::decline()]).contextualized(
            DecisionContext::new(
                seat(),
                DecisionSource::Rule("89.2".to_owned()),
                MOVEMENT_SUBTYPE,
                Phase::Action,
                1,
            ),
        );
        match project_tactical_facts(&state, ContentStore::embedded(), POK, galaxy, &choice) {
            Some(TacticalFacts::Movement(facts)) => facts,
            other => panic!("no movement facts: {other:?}"),
        }
    }

    fn ships<'a>(facts: &'a MovementFacts, origin: &str) -> Vec<&'a ShipFact> {
        let ships: Vec<&ShipFact> = facts
            .ships
            .iter()
            .filter(|ship| ship.origin == origin)
            .collect();
        assert!(!ships.is_empty(), "no ship in {origin}");
        ships
    }

    fn ship<'a>(facts: &'a MovementFacts, origin: &str, unit: &str) -> &'a ShipFact {
        ships(facts, origin)
            .into_iter()
            .find(|ship| ship.unit == unit)
            .unwrap_or_else(|| panic!("no {unit} in {origin}: {:?}", ships(facts, origin)))
    }

    #[test]
    fn an_unknown_scenario_names_the_known_ones() {
        let (mut state, mut galaxy) = game(LATE_GAME);
        let error = apply(
            "nope",
            &mut state,
            &mut galaxy,
            ContentStore::embedded(),
            POK,
        )
        .expect_err("no such scenario");
        assert!(error.contains(LATE_GAME), "{error}");
    }

    #[test]
    fn seat_a_has_the_turn_with_every_ship_type_upgraded() {
        let (state, _) = game(LATE_GAME);
        assert_eq!(state.phase, Phase::Action);
        assert_eq!(state.active, Some(seat()));
        assert!(
            state
                .players
                .iter()
                .all(|seat| seat.strategy_cards.len() == 1)
        );
        assert_eq!(state.tokens_in_reinforcements(&seat()), 0);
        let kinds: std::collections::BTreeSet<String> = state
            .board
            .values()
            .flat_map(|system| system.units_of(&seat()))
            .map(|unit| unit.type_id.to_string())
            .collect();
        for kind in [
            "sol_flagship",
            "warsun",
            "sol_carrier2",
            "cruiser2",
            "destroyer2",
            "dreadnought2",
            "fighter2",
        ] {
            assert!(kinds.contains(kind), "no {kind} in {kinds:?}");
        }
    }

    #[test]
    fn many_fleets_reach_the_middle_and_a_token_holds_one_back() {
        let (state, galaxy) = game(LATE_GAME);
        let facts = facts(&state, &galaxy, at::DEPOT);
        let origins: std::collections::BTreeSet<&str> = facts
            .ships
            .iter()
            .filter(|ship| ship.r#move.is_some())
            .map(|ship| ship.origin.as_str())
            .collect();
        assert!(origins.len() >= 5, "{origins:?}");
        for ship in ships(&facts, at::TOKEN) {
            assert_eq!(ship.blocked, Some(Blocked::CommandToken), "{ship:?}");
        }
        for ship in ships(&facts, at::FAR) {
            assert_eq!(ship.blocked, Some(Blocked::Range), "{ship:?}");
        }
        // Five tokens in the fleet pool, and more ships than that can arrive.
        assert_eq!(facts.fleet_supply.limit, 5);
        let arriving = facts
            .ships
            .iter()
            .filter(|ship| ship.fleet && ship.r#move.is_some())
            .count();
        assert!(arriving > 5, "{arriving}");
    }

    #[test]
    fn the_flagship_needs_gravity_drive_and_loads_on_its_way() {
        let (state, galaxy) = game(LATE_GAME);
        let facts = facts(&state, &galaxy, at::ENEMY_B);
        assert!(facts.gravity_drive);
        let flagship = ship(&facts, at::HOME, "sol_flagship");
        assert_eq!(flagship.move_value, 1);
        let route = flagship.r#move.as_ref().expect("the flagship arrives");
        assert!(route.gravity_drive, "{route:?}");
        assert_eq!(route.path.len(), 3, "{route:?}");
        // The game chooses the path, and the flagship loads in the system that it passes.
        let passed = route.path[1].as_str();
        let systems: std::collections::BTreeSet<&str> = flagship
            .loads
            .iter()
            .map(|index| facts.cargo[*index].system.as_str())
            .collect();
        // Nothing is loaded in a system with a command token of the seat.
        assert!(systems.contains(at::HOME), "{systems:?}");
        assert!(
            systems
                .iter()
                .all(|system| [at::HOME, passed].contains(system))
        );
    }

    #[test]
    fn the_depot_is_on_the_way_of_a_ship() {
        let (state, galaxy) = game(LATE_GAME);
        // Space area and two planets, for some ship that passes: the game chooses the paths.
        let found = [at::ENEMY_B, at::ENEMY_C, at::HOME, at::DOCK, at::TOKEN]
            .into_iter()
            .any(|active| {
                let facts = facts(&state, &galaxy, active);
                facts.ships.iter().any(|ship| {
                    let places: std::collections::BTreeSet<Option<&str>> = ship
                        .loads
                        .iter()
                        .map(|index| &facts.cargo[*index])
                        .filter(|pool| pool.system == at::DEPOT)
                        .map(|pool| pool.source.as_deref())
                        .collect();
                    places.len() == 3
                })
            });
        assert!(found, "no ship passes the depot");
    }

    #[test]
    fn a_fleet_loads_at_three_places_of_its_system() {
        let (state, galaxy) = game(LATE_GAME);
        let facts = facts(&state, &galaxy, at::DEPOT);
        let carrier = ship(&facts, at::DOCK, "sol_carrier2");
        let places: std::collections::BTreeSet<Option<&str>> = carrier
            .loads
            .iter()
            .map(|index| &facts.cargo[*index])
            .filter(|pool| pool.system == at::DOCK)
            .map(|pool| pool.source.as_deref())
            .collect();
        assert_eq!(
            places,
            [None, Some("lisis"), Some("velnor")].into_iter().collect()
        );
    }

    #[test]
    fn the_rift_the_nebula_and_the_supernova_shape_the_paths() {
        let (state, galaxy) = game(LATE_GAME);
        // Out of the rift: every ship that starts there rolls.
        let into_nebula = facts(&state, &galaxy, at::NEBULA);
        for ship in ships(&into_nebula, at::RIFT) {
            let route = ship.r#move.as_ref().expect("a ship leaves the rift");
            assert_eq!(route.rifts, [at::RIFT], "{ship:?}");
        }
        // From the north the rift is on the way, and the other way has the ships of seat H.
        let dreadnought = ship(&into_nebula, at::NORTH, "dreadnought2");
        let route = dreadnought
            .r#move
            .as_ref()
            .expect("the dreadnought arrives");
        assert_eq!(route.path, [at::NORTH, at::RIFT, at::NEBULA]);
        assert_eq!(route.rifts, [at::RIFT]);
        // Out of the nebula a ship moves 1.
        let dock = facts(&state, &galaxy, at::DOCK);
        let nebula = ship(&dock, at::NEBULA, "dreadnought2");
        assert!(nebula.nebula);
        assert!(nebula.r#move.is_some());
        let depot = facts(&state, &galaxy, at::DEPOT);
        assert_eq!(
            ship(&depot, at::NEBULA, "dreadnought2").blocked,
            Some(Blocked::Range)
        );
        // The supernova is the short way from the west to the nebula.
        // The cruiser goes around it, and the destroyer needs Gravity Drive for that.
        for (unit, boosted) in [("cruiser2", false), ("destroyer2", true)] {
            let ship = ship(&into_nebula, at::WEST, unit);
            let route = ship.r#move.as_ref().expect("the ship goes around");
            assert_eq!(route.gravity_drive, boosted, "{ship:?}");
            assert_eq!(route.path.len(), 4, "{ship:?}");
            assert!(!route.path.contains(&at::SUPERNOVA.to_owned()), "{ship:?}");
        }
    }

    #[test]
    fn a_wormhole_brings_the_far_fleet_to_the_north() {
        let (state, galaxy) = game(LATE_GAME);
        let facts = facts(&state, &galaxy, at::NORTH);
        for ship in ships(&facts, at::FAR) {
            let route = ship.r#move.as_ref().expect("the far fleet arrives");
            assert_eq!(route.path.len(), 3, "{ship:?}");
        }
    }

    #[test]
    fn the_carrier_of_the_north_lands_on_two_free_planets() {
        let (state, galaxy) = game(LATE_GAME);
        let content = ContentStore::embedded();
        let free = SystemId::new(at::FREE);
        let planets = ti4_engine::planets::in_system(&state, content, POK, &free);
        assert_eq!(planets.len(), 2);
        let there = state.system_state(&free);
        assert!(there.units.is_empty() && there.planet_units.values().all(Vec::is_empty));

        // The carrier arrives with no roll on its way, and loads on the planet that it leaves.
        let moving = facts(&state, &galaxy, at::FREE);
        let carrier = ship(&moving, at::NORTH, "sol_carrier2");
        let route = carrier.r#move.as_ref().expect("the carrier arrives");
        assert_eq!(route.path, [at::NORTH, at::FREE]);
        assert!(route.rifts.is_empty());
        let loads: Vec<&str> = carrier
            .loads
            .iter()
            .map(|index| moving.cargo[*index].unit.as_str())
            .collect();
        assert!(
            loads.contains(&"sol_infantry") && loads.contains(&"sol_mech"),
            "{loads:?}"
        );

        // In space of the free system, each kind can land on each planet.
        let mut state = state;
        state.active_system = Some(free.clone());
        let kinds = [("sol_infantry", 2), ("sol_mech", 1)];
        for (kind, count) in kinds {
            for _ in 0..count {
                state
                    .system_mut(&free)
                    .units
                    .push(Unit::new(UnitTypeId::new(kind), seat()));
            }
        }
        let mut options = Vec::new();
        for (index, (kind, _)) in kinds.iter().enumerate() {
            for planet in &planets {
                options.push(
                    ChoiceOption::labelled(
                        format!("commit|{index}|{planet}"),
                        ti4_engine::invasion::COMMIT_KIND,
                        "land",
                    )
                    .with("planet", planet.to_string())
                    .with("unit", (*kind).to_owned())
                    .with("damaged", false),
                );
            }
        }
        options.push(ChoiceOption::decline());
        let choice = Choice::new(seat(), "commit", options).contextualized(DecisionContext::new(
            seat(),
            DecisionSource::Rule("49".to_owned()),
            INVASION_SUBTYPE,
            Phase::Action,
            4,
        ));
        let Some(TacticalFacts::Invasion(landing)) =
            project_tactical_facts(&state, content, POK, &galaxy, &choice)
        else {
            panic!("no invasion facts");
        };
        assert_eq!(landing.active, at::FREE);
        let forces: Vec<(&str, usize)> = landing
            .forces
            .iter()
            .map(|force| (force.unit.as_str(), force.count))
            .collect();
        assert_eq!(forces, kinds);
        assert_eq!(landing.planets.len(), 2);
        for planet in &landing.planets {
            assert_eq!(planet.lands, [0, 1], "{planet:?}");
        }
    }

    #[test]
    fn the_exploration_left_attachments_fragments_and_thinner_decks() {
        let (state, galaxy) = game(LATE_GAME);
        let content = ContentStore::embedded();
        let facts = crate::exploration::project_exploration_facts(&state, content, POK, &galaxy);
        let full = crate::exploration::exploration_catalog(content, POK).decks;

        assert_eq!(facts.drawn.len(), 9);
        assert_eq!(facts.decks["CULTURAL"].len() + 5, full["CULTURAL"].len());
        assert_eq!(
            facts.decks["INDUSTRIAL"].len() + 2,
            full["INDUSTRIAL"].len()
        );
        assert!(!facts.decks["CULTURAL"].contains(&"ds".to_owned()));
        let attached = |planet: &str| state.planet_attachments[&PlanetId::new(planet)].clone();
        assert_eq!(attached("vegamajor"), ["dysonsphere"]);
        // Velnor has a technology specialty of its own, Lisis has none.
        assert_eq!(attached("velnor"), ["cyberneticstat"]);
        assert_eq!(attached("lisis"), ["biotic"]);
        assert_eq!(attached("loki"), ["paradiseworld"]);

        let view = crate::projection::project_player_view(state.player(&seat()).unwrap());
        assert_eq!(view.relic_fragments["CULTURAL"], 3);
        assert_eq!(view.relic_fragments["HAZARDOUS"], 1);
        let other =
            crate::projection::project_player_view(state.player(&PlayerId::new("b")).unwrap());
        assert_eq!(other.relic_fragments["FRONTIER"], 1);

        // What seat A used, for the cards of its sheet.
        assert_eq!(
            view.leaders[&LeaderId::new("solagent")],
            LeaderStatus::Exhausted
        );
        assert_eq!(
            view.leaders[&LeaderId::new("solhero")],
            LeaderStatus::Locked
        );
        assert!(
            view.exhausted_technologies
                .contains(&TechnologyId::new("bs"))
        );
        assert_eq!(view.relics.len(), 2);
        assert!(
            view.exhausted_relics
                .contains(&RelicId::new("prophetstears"))
        );
        assert!(!state.relic_deck.contains(&RelicId::new("dynamiscore")));

        // A planet of each deck is free: the two of the free system, one next to seat B, and
        // the one where every other seat has a token.
        let explores = |planet: &str| facts.explores.get(&PlanetId::new(planet)).cloned();
        assert_eq!(explores("abyz"), Some(vec!["HAZARDOUS".to_owned()]));
        assert_eq!(explores("abaddon"), Some(vec!["CULTURAL".to_owned()]));
        assert_eq!(explores("perimeter"), Some(vec!["INDUSTRIAL".to_owned()]));
        assert_eq!(explores("vegamajor"), None);
    }

    #[test]
    fn the_asteroid_field_is_closed_and_the_dreadnought_goes_around() {
        // South to west: through the asteroid field it is 2, around it 3.
        for id in [LATE_GAME, LATE_GAME_DEFLECTORS] {
            let (state, galaxy) = game(id);
            let facts = facts(&state, &galaxy, at::WEST);
            let dreadnought = ship(&facts, at::SOUTH, "dreadnought2");
            let route = dreadnought
                .r#move
                .as_ref()
                .expect("the dreadnought arrives");
            // The engine does not give Antimass Deflectors its effect yet (2026-10-10), so the
            // field is closed with it too. When it does, the second game goes through.
            assert!(route.gravity_drive, "{id}: {route:?}");
            assert!(!route.path.contains(&at::ASTEROIDS.to_owned()), "{id}");
        }
    }

    #[test]
    fn light_wave_deflector_opens_the_home_of_seat_h() {
        let (plain, galaxy) = game(LATE_GAME);
        let (open, _) = game(LATE_GAME_DEFLECTORS);
        // North to the dock: only through the home of seat H, where its ships are.
        let to_dock = |state: &GameState| {
            let facts = facts(state, &galaxy, at::DOCK);
            ship(&facts, at::NORTH, "dreadnought2").clone()
        };
        assert_eq!(to_dock(&plain).blocked, Some(Blocked::Range));
        let route = to_dock(&open).r#move.expect("the dreadnought arrives");
        assert_eq!(route.path.len(), 3, "{route:?}");
        assert!(route.rifts.is_empty());
    }
}
