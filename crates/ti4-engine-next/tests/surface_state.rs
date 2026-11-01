//! Lane B (8d items 2 and 3, docs/D28B2_RESUME.md): the production tail and the state queries answer as the legacy originals do, on the
//! positions of `surface_common` and on variants of them (technologies researched, planets exhausted, laws and attachments on planets).

mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus};

fn players(state: &GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

fn unit_upgrades(sources: ti4_model::content_types::SourceSet) -> Vec<TechnologyId> {
    old::technology::active_aliases(content(), sources).into_iter().filter(|t| old::technology::is_unit_upgrade(content(), t)).collect()
}

#[test]
fn buildable_for_answers_as_the_legacy_one_does() {
    let content = content();
    let mut compared = 0_usize;
    let mut distinct = std::collections::BTreeSet::new();
    for pos in positions() {
        let upgrades = unit_upgrades(pos.sources);
        assert!(upgrades.len() > 5, "the corpus has unit upgrades");
        for player in players(&pos.state) {
            // The position as it stands; then with the war sun technology; then with each unit upgrade; then with all of them.
            let mut variants: Vec<(String, Vec<TechnologyId>)> = vec![("as is".to_owned(), Vec::new()), ("ws".to_owned(), vec![TechnologyId::new("ws")])];
            variants.extend(upgrades.iter().map(|t| (t.as_str().to_owned(), vec![t.clone()])));
            variants.push(("all".to_owned(), upgrades.clone()));
            for (label, techs) in variants {
                let mut state = pos.state.clone();
                state.player_mut(&player).expect("a seat").technologies.extend(techs);
                let (a, b) = (old::production::buildable_for(&state, content, pos.sources, &player), thin::queries::production::buildable_for(&state, content, pos.sources, &player));
                assert_eq!(a, b, "{}: buildable_for {player} with {label}", pos.label);
                distinct.insert(a);
                compared += 1;
            }
        }
    }
    assert!(compared > 500, "only {compared} compared");
    assert!(distinct.len() > 8, "only {} distinct answers: the corpus does not vary what is buildable", distinct.len());
}

#[test]
fn the_payment_planets_answer_as_the_legacy_ones_do() {
    use thin::queries::production::Spend as T;
    let content = content();
    let mut listed = 0_usize;
    for pos in positions() {
        for player in players(&pos.state) {
            // As is; half the planets exhausted; Xxekir Grom unlocked (a planet pays its two values together).
            let mut halved = pos.state.clone();
            let owned: Vec<PlanetId> = halved.controlled_planets(&player).into_iter().map(|(_, p)| p.clone()).collect();
            for planet in owned.iter().step_by(2) {
                halved.exhaust_planet(planet.clone());
            }
            let mut combined = pos.state.clone();
            combined.player_mut(&player).expect("a seat").leaders.insert(LeaderId::new("xxchahero"), LeaderStatus::Unlocked);
            for (label, state) in [("as is", &pos.state), ("half exhausted", &halved), ("xxekir grom", &combined)] {
                for (o, n) in [(old::production::Spend::Resources, T::Resources), (old::production::Spend::Influence, T::Influence)] {
                    let a = old::production::native_payment_planets(state, content, pos.sources, &player, o);
                    let b = thin::queries::production::native_payment_planets(state, content, pos.sources, &player, n);
                    assert_eq!(a, b, "{}: native_payment_planets {player} {o:?} ({label})", pos.label);
                    listed += a.len();
                }
            }
        }
    }
    assert!(listed > 200, "only {listed} planets listed");
}

#[test]
fn the_value_boosts_answer_as_the_legacy_ones_do() {
    let content = content();
    let attachments: Vec<String> = content
        .records(ti4_model::content_types::ContentType::Attachments)
        .iter()
        .filter(|r| r.int("resourcesModifier").unwrap_or(0) != 0 || r.int("influenceModifier").unwrap_or(0) != 0)
        .filter_map(|r| r.text("id").map(ToOwned::to_owned))
        .collect();
    assert!(attachments.len() > 3, "the corpus has attachments that move a value");
    let (mut boosts, mut with_law, mut with_attachment, mut helios) = (0_usize, 0_usize, 0_usize, 0_usize);
    for pos in positions() {
        let planets: Vec<PlanetId> = pos.state.board.values().flat_map(|s| s.planet_control.keys().cloned()).collect();
        for planet in &planets {
            let mut variants: Vec<(String, GameState)> = vec![("as is".to_owned(), pos.state.clone())];
            for law in ["core_mining", "senate_sanctuary", "terraforming_initiative"] {
                let mut s = pos.state.clone();
                s.laws.insert(law.to_owned(), planet.as_str().to_owned());
                variants.push((law.to_owned(), s));
            }
            let mut both = pos.state.clone();
            for law in ["core_mining", "senate_sanctuary", "terraforming_initiative"] {
                both.laws.insert(law.to_owned(), planet.as_str().to_owned());
            }
            both.planet_attachments.insert(planet.clone(), attachments.iter().take(2).cloned().collect());
            variants.push(("three laws and two attachments".to_owned(), both));
            let mut attached = pos.state.clone();
            attached.planet_attachments.insert(planet.clone(), attachments.clone());
            variants.push(("every attachment".to_owned(), attached));
            // A Last Bastion seat with a Helios dock (V1, then V2) on the planet: +1 / +2 resources.
            for (dock, label) in [("bastion_spacedock", "helios 1"), ("bastion_spacedock2", "helios 2")] {
                let mut s = pos.state.clone();
                s.players[0].faction = ti4_model::id::FactionId::new("bastion");
                let owner = s.players[0].id.clone();
                let system = s.board.iter().find(|(_, b)| b.planet_control.contains_key(planet)).map(|(id, _)| id.clone()).expect("a system");
                s.board.get_mut(&system).expect("a system").planet_units.entry(planet.clone()).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(dock), owner));
                variants.push((label.to_owned(), s));
            }
            for (label, state) in variants {
                let (a, b) = (old::production::planet_value_boosts(&state, content, planet), thin::queries::production::planet_value_boosts(&state, content, planet));
                assert_eq!(a, b, "{}: planet_value_boosts {planet} ({label})", pos.label);
                boosts += a.len();
                with_law += usize::from(a.iter().any(|(name, _, _)| matches!(name.as_str(), "Core Mining" | "Senate Sanctuary" | "Terraforming Initiative")));
                with_attachment += usize::from(a.len() > 3);
                helios += usize::from(a.iter().any(|(name, _, _)| name == "Helios dock"));
            }
        }
    }
    assert!(boosts > 300 && with_law > 100 && with_attachment > 20, "boosts {boosts}, with a law {with_law}, with attachments {with_attachment}");
    assert!(helios > 20, "only {helios} Helios docks listed");
}

fn dbg<T: std::fmt::Debug>(t: &T) -> String {
    format!("{t:?}")
}

fn systems_of(pos: &surface_common::Pos) -> Vec<ti4_model::id::SystemId> {
    pos.galaxy.system_ids().into_iter().map(ti4_model::id::SystemId::new).collect()
}

#[test]
fn the_planet_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    let attachments: Vec<String> = content.records(ti4_model::content_types::ContentType::Attachments).iter().filter_map(|r| r.text("id").map(ToOwned::to_owned)).collect();
    assert!(attachments.len() > 10);
    let (mut planets, mut attached_specialties, mut attached_traits) = (0_usize, 0_usize, 0_usize);
    for pos in positions() {
        for system in systems_of(&pos) {
            assert_eq!(
                old::planets::in_system(&pos.state, content, pos.sources, &system),
                thin::queries::planets::in_system(&pos.state, content, pos.sources, &system),
                "{}: in_system {system}",
                pos.label
            );
            for planet in old::planets::in_system(&pos.state, content, pos.sources, &system) {
                let mut variants = vec![("as is".to_owned(), pos.state.clone())];
                for id in &attachments {
                    let mut s = pos.state.clone();
                    s.planet_attachments.insert(planet.clone(), vec![id.clone()]);
                    variants.push((id.clone(), s));
                }
                for (label, state) in variants {
                    let (a, b) = (old::planets::tech_specialties_now(&state, content, pos.sources, &planet), thin::queries::planets::tech_specialties_now(&state, content, pos.sources, &planet));
                    assert_eq!(a, b, "{}: tech_specialties_now {planet} ({label})", pos.label);
                    let printed = old::planets::tech_specialties_now(&pos.state, content, pos.sources, &planet);
                    attached_specialties += usize::from(a != printed);
                    let (a, b) = (old::planets::traits_now(&state, content, pos.sources, &planet), thin::queries::planets::traits_now(&state, content, pos.sources, &planet));
                    assert_eq!(a, b, "{}: traits_now {planet} ({label})", pos.label);
                    attached_traits += usize::from(a != old::planets::traits_now(&pos.state, content, pos.sources, &planet));
                    planets += 1;
                }
            }
        }
        // A planet placed into a system by a card (the placed-planets map) is listed there too.
        let mut placed = pos.state.clone();
        let target = systems_of(&pos).remove(0);
        placed.placed_planets.insert(PlanetId::new("mirage"), target.clone());
        assert_eq!(old::planets::in_system(&placed, content, pos.sources, &target), thin::queries::planets::in_system(&placed, content, pos.sources, &target), "{}: a placed planet", pos.label);
        assert!(thin::queries::planets::in_system(&placed, content, pos.sources, &target).contains(&PlanetId::new("mirage")));
    }
    assert!(planets > 2000 && attached_specialties > 20 && attached_traits > 20, "planets {planets}, with attached specialties {attached_specialties}, traits {attached_traits}");
}

#[test]
fn the_supply_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    for base in ["flagship", "warsun", "dreadnought", "cruiser", "destroyer", "carrier", "mech", "pds", "spacedock", "fighter", "infantry", "nothing"] {
        assert_eq!(old::supply::plastic(base), thin::queries::supply::plastic(base), "plastic {base}");
    }
    let units: Vec<ti4_model::id::UnitTypeId> = ti4_content::units::catalogue(content, ti4_model::content_types::POK).keys().map(|id| ti4_model::id::UnitTypeId::new(*id)).collect();
    assert!(units.len() > 30);
    let (mut capped, mut spent) = (0_usize, 0_usize);
    for pos in positions() {
        let mut crimson = pos.state.clone();
        let owner = crimson.players[0].id.clone();
        crimson.faction_marks.insert(format!("crimson:card:{owner}"), "cruiser,cruiser,dreadnought,".to_owned());
        let first = crimson.players[0].id.clone();
        crimson.players[1].captured_units.push((first, ti4_model::id::UnitTypeId::new("carrier")));
        for (label, state) in [("as is", &pos.state), ("ships on the Crimson card and a captured unit", &crimson)] {
            for player in players(state) {
                for unit in &units {
                    let (a, b) = (old::supply::remaining(state, content, pos.sources, &player, unit), thin::queries::supply::remaining(state, content, pos.sources, &player, unit));
                    assert_eq!(a, b, "{}: remaining {player} {unit} ({label})", pos.label);
                    capped += usize::from(a < 99);
                    spent += usize::from(a < 99 && b < old::supply::plastic(&old::supply::base_type_of(content, pos.sources, unit)).unwrap_or(99));
                }
                for base in ["cruiser", "carrier", "dreadnought", "pds"] {
                    assert_eq!(old::supply::held(state, content, pos.sources, &player, base), thin::queries::supply::held(state, content, pos.sources, &player, base), "{}: held {player} {base} ({label})", pos.label);
                }
            }
        }
    }
    assert!(capped > 1000 && spent > 200, "capped {capped}, with some out of the box {spent}");
}

#[test]
fn the_combat_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    let key = |units: &[ti4_model::units::Unit]| units.iter().map(|u| format!("{} {} {} {}", u.type_id, u.owner, u.sustained_damage, u.galvanized)).collect::<Vec<_>>();
    let (mut with_guns, mut with_attachments, mut opponents, mut hits) = (0_usize, 0_usize, 0_usize, 0_usize);
    for pos in positions() {
        // The corpus rarely has a PDS beside an enemy fleet: every seat also gets a PDS II on a planet it holds and a cruiser of the next seat in
        // the same system, and Shields Holding cards.
        let mut armed = pos.state.clone();
        let seats = players(&pos.state);
        for (i, player) in seats.iter().enumerate() {
            let planets: Vec<(ti4_model::id::SystemId, PlanetId)> = armed.controlled_planets(player).into_iter().map(|(s, p)| (s.clone(), p.clone())).collect();
            if let Some((system, planet)) = planets.first() {
                armed.board.get_mut(system).expect("a system").planet_units.entry(planet.clone()).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("pds2"), player.clone()));
                armed.board.get_mut(system).expect("a system").units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("cruiser"), seats[(i + 1) % seats.len()].clone()));
            }
            armed.player_mut(player).expect("a seat").cancel_hits_round = Some((armed.combat_round_seq, i + 1));
        }
        for (label, state) in [("as is", &pos.state), ("armed", &armed)] {
            for player in &seats {
                assert_eq!(old::combat::cancellable_hits(state, player), thin::queries::combat::cancellable_hits(state, player), "{}: cancellable_hits {player} ({label})", pos.label);
                hits += old::combat::cancellable_hits(state, player);
                for system in systems_of(&pos) {
                    let (a, b) = (old::combat::opponent_with_ships(state, content, pos.sources, player, &system), thin::queries::combat::opponent_with_ships(state, content, pos.sources, player, &system));
                    assert_eq!(a, b, "{}: opponent_with_ships {player} {system} ({label})", pos.label);
                    opponents += usize::from(a.is_some());
                    for galaxy in [None, Some(&pos.galaxy)] {
                        let (a, b) = (old::combat::space_cannon_guns(state, content, pos.sources, &system, player, galaxy), thin::queries::combat::space_cannon_guns(state, content, pos.sources, &system, player, galaxy));
                        assert_eq!((key(&a.units), dbg(&a.attachments)), (key(&b.units), dbg(&b.attachments)), "{}: space_cannon_guns {player} {system} map {} ({label})", pos.label, galaxy.is_some());
                        with_guns += usize::from(!a.units.is_empty());
                        with_attachments += usize::from(!a.attachments.is_empty());
                    }
                }
            }
        }
    }
    assert!(with_guns > 100 && opponents > 100 && hits > 20, "guns {with_guns}, opponents {opponents}, hits {hits}, attachments {with_attachments}");
}

#[test]
fn bombardable_answers_as_the_legacy_one_does() {
    let content = content();
    let (mut yes, mut no) = (0_usize, 0_usize);
    for pos in positions() {
        // Each seat also gets a war sun in the systems it holds a planet in (a war sun ignores Planetary Shield) as a variant.
        let mut sun = pos.state.clone();
        for player in players(&pos.state) {
            for (system, _) in pos.state.controlled_planets(&player) {
                sun.board.get_mut(system).expect("a system").units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("warsun"), player.clone()));
            }
        }
        for (label, state) in [("as is", &pos.state), ("war suns", &sun)] {
            for invader in players(state) {
                for (system, board) in &state.board {
                    for planet in board.planet_control.keys() {
                        let (a, b) = (old::invasion::bombardable(state, content, pos.sources, system, planet, &invader), thin::queries::invasion::bombardable(state, content, pos.sources, system, planet, &invader));
                        assert_eq!(a, b, "{}: bombardable {invader} {system} {planet} ({label})", pos.label);
                        if a { yes += 1 } else { no += 1 }
                    }
                }
            }
        }
    }
    assert!(yes > 50 && no > 50, "yes {yes}, no {no}");
}

#[test]
fn the_legendary_and_scar_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    let all = ti4_content::galaxy::all_systems(content, ti4_model::content_types::FULL);
    let mut scars = 0_usize;
    for (id, _) in all.iter() {
        let system = ti4_model::id::SystemId::new(*id);
        let (a, b) = (old::entropic_scars::is_scar(content, ti4_model::content_types::FULL, &system), thin::queries::entropic_scars::is_scar(content, ti4_model::content_types::FULL, &system));
        assert_eq!(a, b, "is_scar {id}");
        scars += usize::from(a);
    }
    assert!(scars >= 2, "only {scars} scars in the corpus");
    let ids: Vec<ti4_model::id::SystemId> = all.iter().map(|(id, _)| ti4_model::id::SystemId::new(*id)).collect();
    for from in &ids {
        for against in std::iter::once(None).chain(ids.iter().step_by(7).map(Some)) {
            assert_eq!(
                old::entropic_scars::abilities_usable(content, ti4_model::content_types::FULL, from, against),
                thin::queries::entropic_scars::abilities_usable(content, ti4_model::content_types::FULL, from, against),
                "abilities_usable {from} -> {against:?}"
            );
        }
    }
    let (mut held, mut spent) = (0_usize, 0_usize);
    for pos in positions() {
        for player in players(&pos.state) {
            let owned: Vec<PlanetId> = pos.state.controlled_planets(&player).into_iter().map(|(_, p)| p.clone()).collect();
            let mut states = vec![pos.state.clone()];
            let mut with_ionian = pos.state.clone();
            let tempesta = PlanetId::new("tempesta");
            let system = with_ionian.board.keys().next().expect("a system").clone();
            with_ionian.board.get_mut(&system).expect("a system").planet_control.insert(tempesta.clone(), player.clone());
            states.push(with_ionian.clone());
            with_ionian.player_mut(&player).expect("a seat").exhausted_legendary.insert(tempesta.clone());
            states.push(with_ionian);
            for state in &states {
                assert_eq!(old::legendary::ionian_available(state, &player), thin::queries::legendary::ionian_available(state, &player), "{}: ionian_available {player}", pos.label);
                held += usize::from(old::legendary::ionian_available(state, &player));
                spent += usize::from(state.player(&player).is_some_and(|s| s.exhausted_legendary.contains(&tempesta)));
                for planet in owned.iter().chain(std::iter::once(&tempesta)) {
                    assert_eq!(old::legendary::available(state, &player, planet), thin::queries::legendary::available(state, &player, planet), "{}: available {player} {planet}", pos.label);
                }
            }
        }
    }
    assert!(held > 5 && spent > 5, "held {held}, spent {spent}");
}

#[test]
fn faction_abilities_answer_as_the_legacy_ones_do() {
    let content = content();
    let abilities: std::collections::BTreeSet<String> = content
        .records(ti4_model::content_types::ContentType::Factions)
        .iter()
        .flat_map(|r| r.strings("abilities").into_iter().map(ToOwned::to_owned).collect::<Vec<_>>())
        .chain(["no_such_ability".to_owned(), String::new()])
        .collect();
    assert!(abilities.len() > 40);
    let mut yes = 0_usize;
    for pos in positions() {
        for player in players(&pos.state) {
            for ability in &abilities {
                let (a, b) = (old::faction_abilities::has(&pos.state, content, &player, ability), thin::queries::faction_abilities::has(&pos.state, content, &player, ability));
                assert_eq!(a, b, "{}: has {player} {ability}", pos.label);
                yes += usize::from(a);
            }
        }
    }
    assert!(yes > 20, "only {yes} abilities held");
}

#[test]
fn the_fleet_standing_answers_as_the_legacy_one_does() {
    let content = content();
    let types = ti4_content::units::catalogue(content, ti4_model::content_types::POK);
    let (mut charged, mut excess, mut arrivals) = (0_usize, 0_usize, 0_usize);
    for pos in positions() {
        // Also a state with the fleet pool capped by Fleet Regulations, and one with a fighter upgrade owned.
        let mut capped = pos.state.clone();
        capped.laws.insert("regulations".to_owned(), String::new());
        let mut upgraded = pos.state.clone();
        for p in &mut upgraded.players {
            p.technologies.insert(TechnologyId::new("ff2"));
        }
        // Every seat stacks six cruisers, four carriers and five infantry in the first system it has a unit in: over the fleet limit and the capacity.
        let mut stacked = pos.state.clone();
        for player in players(&pos.state) {
            let Some(system) = stacked.board.iter().find(|(_, b)| b.units.iter().any(|u| u.owner == player)).map(|(id, _)| id.clone()) else { continue };
            let board = stacked.board.get_mut(&system).expect("a system");
            for (kind, n) in [("cruiser", 6), ("carrier", 4), ("infantry", 5), ("fighter", 6)] {
                for _ in 0..n {
                    board.units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), player.clone()));
                }
            }
        }
        for (label, state) in [("as is", &pos.state), ("fleet regulations", &capped), ("fighter ii", &upgraded), ("stacked", &stacked)] {
            for player in players(state) {
                for system in systems_of(&pos) {
                    let a = old::fleet::standing(state, content, pos.sources, &player, &system, None);
                    let b = thin::queries::fleet::standing(state, content, pos.sources, &player, &system, None);
                    assert_eq!(dbg(&a), dbg(&b), "{}: standing {player} {system} ({label})", pos.label);
                    charged += usize::from(a.fleet_charged > 0);
                    excess += usize::from(a.fleet_excess() > 0 || a.capacity_excess > 0);
                    if a.ships == 0 && a.fighters == 0 {
                        continue;
                    }
                    for kind in ["fighter", "infantry", "cruiser", "dreadnought", "carrier"] {
                        let unit = types.get(kind).expect("a unit");
                        for in_space in [true, false] {
                            let a = old::fleet::standing(state, content, pos.sources, &player, &system, Some(old::fleet::Arrival { kind: *unit, count: 2, in_space }));
                            let b = thin::queries::fleet::standing(state, content, pos.sources, &player, &system, Some(thin::queries::fleet::Arrival { kind: *unit, count: 2, in_space }));
                            assert_eq!(dbg(&a), dbg(&b), "{}: standing {player} {system} + 2 {kind} space {in_space} ({label})", pos.label);
                            arrivals += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(charged > 100 && excess > 20 && arrivals > 500, "charged {charged}, excess {excess}, arrivals {arrivals}");
}

#[test]
fn the_faction_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    let factions: Vec<String> = content.records(ti4_model::content_types::ContentType::Factions).iter().filter_map(|r| r.text("id").or_else(|| r.text("alias")).map(ToOwned::to_owned)).chain(["keleresm".to_owned(), "keleresx".to_owned(), "keleresa".to_owned(), "nobody".to_owned()]).collect();
    assert!(factions.len() > 25);
    let mut listed = 0_usize;
    for sources in [ti4_model::content_types::POK, ti4_model::content_types::FULL] {
        for faction in &factions {
            let a = old::factions::assets(content, sources, faction);
            let b = thin::queries::factions::assets(content, sources, faction);
            assert_eq!(a.iter().map(|x| (format!("{:?}", x.kind), x.id.clone())).collect::<Vec<_>>(), b.iter().map(|x| (format!("{:?}", x.kind), x.id.clone())).collect::<Vec<_>>(), "assets {faction}");
            listed += a.len();
        }
    }
    assert!(listed > 800, "only {listed} assets");
    let mut reduced = 0_usize;
    for pos in positions() {
        for player in players(&pos.state) {
            let mut marked = pos.state.clone();
            marked.faction_marks.insert(format!("mahact:fleet:{player}"), "b,c,b,".to_owned());
            assert_eq!(old::factions::mahact::fleet_pool_owners(&marked, &player), thin::queries::factions::fleet_pool_owners(&marked, &player), "{}: fleet_pool_owners", pos.label);
            assert_eq!(thin::queries::factions::fleet_pool_owners(&marked, &player).len(), 3);
            assert_eq!(old::factions::mahact::fleet_pool_owners(&pos.state, &player), thin::queries::factions::fleet_pool_owners(&pos.state, &player));
            for system in systems_of(&pos) {
                assert_eq!(
                    old::factions::hooks_economy::production_cost_reduction(&pos.state, &player, &system),
                    thin::queries::factions::production_cost_reduction(&pos.state, &player, &system),
                    "{}: production_cost_reduction {player} {system}",
                    pos.label
                );
            }
        }
    }
    // The Ghosts of Creuss with Particle Synthesis: -1 per wormhole in the system (a table with a Ghosts seat on a real map).
    let ids = [PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c")];
    let assignments: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId> = [("a", "ghost"), ("b", "sol"), ("c", "hacan")].into_iter().map(|(p, f)| (PlayerId::new(p), ti4_model::id::FactionId::new(f))).collect();
    let (mut ghosts, _) = ti4_host::map::create_game_with_template_seated(content, &ids, 4, None, &assignments, None).expect("a map");
    let plain = ghosts.clone();
    ghosts.player_mut(&ids[0]).expect("a seat").breakthrough = Some(ti4_model::id::BreakthroughId::new("ghostbt"));
    for (label, state) in [("without the breakthrough", &plain), ("particle synthesis", &ghosts)] {
        for player in &ids {
            // Every system of the corpus (the random boards hold few wormholes).
            for system in ti4_content::galaxy::all_systems(content, ti4_model::content_types::POK).keys().map(|id| SystemId::new(*id)) {
                let (a, b) = (old::factions::hooks_economy::production_cost_reduction(state, player, &system), thin::queries::factions::production_cost_reduction(state, player, &system));
                assert_eq!(a, b, "production_cost_reduction {player} {system} ({label})");
                reduced += usize::from(a > 0);
            }
        }
    }
    assert!(reduced > 0, "no cost reduction anywhere in the corpus");
}

#[test]
fn the_promissory_reads_answer_as_the_legacy_ones_do() {
    let mut notes = 0_usize;
    for (alias, owner) in [("an", "mahact"), ("ms", "sol"), ("x", ""), ("a:b:c", "")] {
        assert_eq!(old::promissory::note_id(alias, owner), thin::queries::promissory::note_id(alias, owner));
        assert_eq!(old::promissory::alias_of(&format!("{alias}:{owner}")), thin::queries::promissory::alias_of(&format!("{alias}:{owner}")));
    }
    for note in ["support_for_the_throne:sol", "ms:sol", "plain", "a:b:c", "support_for_the_throne:"] {
        assert_eq!(old::promissory::owner_of(note), thin::queries::promissory::owner_of(note), "owner_of {note}");
    }
    for pos in positions() {
        for player in players(&pos.state) {
            assert_eq!(old::promissory::faction_name(&pos.state, &player), thin::queries::promissory::faction_name(&pos.state, &player));
            let held = old::promissory::held_by(&pos.state, &player);
            assert_eq!(held, thin::queries::promissory::held_by(&pos.state, &player), "{}: held_by", pos.label);
            notes += held.len();
            for note in pos.state.promissory_notes.keys().take(40).map(String::as_str).chain(["an:mahact", "an:sol"]) {
                assert_eq!(old::promissory::may_receive(&pos.state, &player, note), thin::queries::promissory::may_receive(&pos.state, &player, note), "{}: may_receive {player} {note}", pos.label);
            }
            for alias in ["an", "ms", "convoys", "support"] {
                for owner in players(&pos.state) {
                    assert_eq!(old::promissory::holder_of(&pos.state, alias, &owner), thin::queries::promissory::holder_of(&pos.state, alias, &owner), "{}: holder_of {alias}", pos.label);
                }
            }
        }
    }
    assert!(notes > 50, "only {notes} notes");
}

#[test]
fn the_card_and_value_reads_answer_as_the_legacy_ones_do() {
    let content = content();
    let cards: Vec<String> = content.records(ti4_model::content_types::ContentType::StrategyCards).iter().filter_map(|r| r.text("alias").or_else(|| r.text("id")).map(ToOwned::to_owned)).chain(["nope".to_owned()]).collect();
    assert!(cards.len() > 8);
    for card in &cards {
        assert_eq!(old::strategy_cards::card_kind(content, card), thin::queries::strategy_cards::card_kind(content, card), "card_kind {card}");
    }
    let units: Vec<String> = ti4_content::units::catalogue(content, ti4_model::content_types::FULL).keys().map(|id| (*id).to_owned()).chain(["nope".to_owned()]).collect();
    assert!(units.len() > 40);
    for sources in [ti4_model::content_types::POK, ti4_model::content_types::FULL] {
        for unit in &units {
            assert_eq!(old::valuation::unit_value(content, sources, unit).to_bits(), thin::queries::valuation::unit_value(content, sources, unit).to_bits(), "unit_value {unit}");
        }
    }
    let mut frontier = 0_usize;
    for pos in positions() {
        assert_eq!(old::exploration::frontier_systems(content, pos.sources, &pos.galaxy), thin::queries::content::frontier_systems(content, pos.sources, &pos.galaxy), "{}: frontier_systems", pos.label);
        frontier += thin::queries::content::frontier_systems(content, pos.sources, &pos.galaxy).len();
        for player in players(&pos.state) {
            let mut sanctions = pos.state.clone();
            sanctions.laws.insert("sanctions".to_owned(), String::new());
            let mut peek = pos.state.clone();
            peek.faction_marks.insert(old::action_cards::insider_peek_key(&player), peek.agenda_deck.iter().take(3).map(String::as_str).collect::<Vec<_>>().join(","));
            let mut stale = peek.clone();
            stale.agenda_deck.rotate_left(1);
            for (label, state) in [("as is", &pos.state), ("sanctions", &sanctions)] {
                assert_eq!(old::action_cards::hand_limit(state, content, &player), thin::queries::action_cards::hand_limit(state, content, &player), "{}: hand_limit {player} ({label})", pos.label);
            }
            assert_eq!(thin::queries::action_cards::hand_limit(&sanctions, content, &player), 3);
            assert_eq!(thin::queries::action_cards::hand_limit(&pos.state, content, &player), old::action_cards::hand_limit(&pos.state, content, &player));
            for (label, state) in [("none", &pos.state), ("fresh", &peek), ("stale", &stale)] {
                assert_eq!(old::action_cards::insider_peek(state, &player), thin::queries::action_cards::insider_peek(state, &player), "{}: insider_peek {player} ({label})", pos.label);
            }
            assert_eq!(thin::queries::action_cards::insider_peek(&peek, &player).len(), peek.agenda_deck.len().min(3));
            assert!(thin::queries::action_cards::insider_peek(&stale, &player).is_empty());
            assert_eq!(old::action_cards::insider_peek_key(&player), thin::queries::action_cards::insider_peek_key(&player));
            assert!(thin::queries::action_cards::is_insider_peek_key(&thin::queries::action_cards::insider_peek_key(&player)));
        }
    }
    // The random boards of the corpus have no planetless tile: a hub whose centre is one (and whose ring is ordinary systems).
    let all = ti4_content::galaxy::all_systems(content, ti4_model::content_types::POK);
    let planetless = all.iter().find(|(id, s)| s.planets().is_empty() && s.is_anomaly() && !["18", "17"].contains(*id)).map(|(id, _)| (*id).to_owned()).expect("a planetless tile");
    let hub = old::fixtures::hub_with_centre(&planetless);
    let (a, b) = (old::exploration::frontier_systems(content, ti4_model::content_types::POK, &hub.galaxy), thin::queries::content::frontier_systems(content, ti4_model::content_types::POK, &hub.galaxy));
    assert_eq!(a, b, "frontier_systems on a hub around {planetless}");
    assert!(!b.is_empty() && b.contains(&SystemId::new(planetless)), "the planetless centre takes a frontier token");
    let _ = frontier;
    for key in ["private:a:insider:peek", "private:a:other", "x:insider:peek", "private:insider:peek"] {
        assert_eq!(old::action_cards::is_insider_peek_key(key), thin::queries::action_cards::is_insider_peek_key(key), "{key}");
    }
}

#[test]
fn scoreable_on_answers_as_the_legacy_one_does() {
    let content = content();
    let mut some = 0_usize;
    for pos in positions() {
        for galaxy in [None, Some(&pos.galaxy)] {
            for player in players(&pos.state) {
                let (a, b) = (old::objectives::scoreable_on(&pos.state, content, pos.sources, &player, galaxy), thin::queries::objectives::scoreable_on(&pos.state, content, pos.sources, &player, galaxy));
                assert_eq!(a, b, "{}: scoreable_on {player} map {}", pos.label, galaxy.is_some());
                some += usize::from(!a.is_empty());
            }
        }
    }
    assert!(some > 3, "only {some} non-empty answers");
}

/// A three-seat table on a real map whose first seat plays `faction`, armed so the faction hooks have something to answer: a PDS II of the
/// second seat, a war sun and a flagship of the first on its planets and in its space, fighters and infantry, and every unit upgrade researched.
fn armed_table(faction: &str, seed: u64) -> (GameState, ti4_content::galaxy::Galaxy) {
    let content = content();
    let ids = [PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c")];
    let others: Vec<&str> = ["sol", "hacan", "letnev"].into_iter().filter(|other| *other != faction).take(2).collect();
    let assignments: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId> = [("a", faction), ("b", others[0]), ("c", others[1])].into_iter().map(|(p, f)| (PlayerId::new(p), ti4_model::id::FactionId::new(f))).collect();
    let (mut state, galaxy) = (seed..seed + 60).find_map(|seed| ti4_host::map::create_game_with_template_seated(content, &ids, seed, None, &assignments, None).ok()).unwrap_or_else(|| panic!("a map for some seed with {faction}: {:?}", ti4_host::map::create_game_with_template_seated(content, &ids, seed, None, &assignments, None).err()));
    let upgrades = unit_upgrades(ti4_model::content_types::POK);
    state.player_mut(&ids[0]).expect("a seat").technologies.extend(upgrades);
    let home = state.board.iter().find(|(_, b)| b.units.iter().any(|u| u.owner == ids[0])).map(|(id, _)| id.clone()).expect("a home");
    let unit = |kind: &str, owner: &PlayerId| ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), owner.clone());
    let board = state.board.get_mut(&home).expect("a home");
    for kind in ["warsun", "dreadnought", "fighter", "fighter", "fighter", "infantry", "carrier"] {
        board.units.push(unit(kind, &ids[0]));
    }
    board.units.push(unit("cruiser", &ids[1]));
    if let Some(planet) = board.planet_control.keys().next().cloned() {
        board.planet_units.entry(planet.clone()).or_default().push(unit("pds2", &ids[1]));
        board.planet_units.entry(planet).or_default().push(unit("mech", &ids[0]));
    }
    (state, galaxy)
}

#[test]
fn every_lobby_faction_answers_as_the_legacy_one_does() {
    let content = content();
    let sources = ti4_model::content_types::POK;
    let key = |units: &[ti4_model::units::Unit]| units.iter().map(|u| format!("{} {} {} {}", u.type_id, u.owner, u.sustained_damage, u.galvanized)).collect::<Vec<_>>();
    let (mut tables, mut guns, mut bombarded, mut abilities_held) = (0_usize, 0_usize, 0_usize, 0_usize);
    for (n, faction) in old::seating::LOBBY_FACTIONS.iter().enumerate() {
        let (state, galaxy) = armed_table(faction, 3 + n as u64);
        let ids = players(&state);
        let all_abilities: Vec<String> = ti4_content::factions::get(content, faction).map(|f| f.abilities().iter().map(|a| (*a).to_owned()).collect()).unwrap_or_default();
        let at = |what: &str| format!("{faction}: {what}");
        for player in &ids {
            assert_eq!(dbg(&old::production::buildable_for(&state, content, sources, player)), dbg(&thin::queries::production::buildable_for(&state, content, sources, player)), "{}", at(&format!("buildable_for {player}")));
            assert_eq!(old::factions::hooks_economy::production_cost_reduction(&state, player, &SystemId::new("17")), thin::queries::factions::production_cost_reduction(&state, player, &SystemId::new("17")), "{}", at("production_cost_reduction"));
            assert_eq!(old::action_cards::hand_limit(&state, content, player), thin::queries::action_cards::hand_limit(&state, content, player), "{}", at("hand_limit"));
            assert_eq!(old::factions::mahact::fleet_pool_owners(&state, player), thin::queries::factions::fleet_pool_owners(&state, player), "{}", at("fleet_pool_owners"));
            for ability in &all_abilities {
                let (a, b) = (old::faction_abilities::has(&state, content, player, ability), thin::queries::faction_abilities::has(&state, content, player, ability));
                assert_eq!(a, b, "{}", at(&format!("has {player} {ability}")));
                abilities_held += usize::from(a);
            }
            for system in galaxy.system_ids().into_iter().map(SystemId::new) {
                let (a, b) = (old::fleet::standing(&state, content, sources, player, &system, None), thin::queries::fleet::standing(&state, content, sources, player, &system, None));
                assert_eq!(dbg(&a), dbg(&b), "{}", at(&format!("standing {player} {system}")));
                for galaxy in [None, Some(&galaxy)] {
                    let (a, b) = (old::combat::space_cannon_guns(&state, content, sources, &system, player, galaxy), thin::queries::combat::space_cannon_guns(&state, content, sources, &system, player, galaxy));
                    assert_eq!((key(&a.units), dbg(&a.attachments)), (key(&b.units), dbg(&b.attachments)), "{}", at(&format!("space_cannon_guns {player} {system} map {}", galaxy.is_some())));
                    guns += a.units.len() + a.attachments.len();
                }
                for planet in state.board.get(&system).map(|b| b.planet_control.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                    let (a, b) = (old::invasion::bombardable(&state, content, sources, &system, &planet, player), thin::queries::invasion::bombardable(&state, content, sources, &system, &planet, player));
                    assert_eq!(a, b, "{}", at(&format!("bombardable {player} {planet}")));
                    bombarded += usize::from(a);
                }
            }
            for galaxy in [None, Some(&galaxy)] {
                assert_eq!(old::objectives::scoreable_on(&state, content, sources, player, galaxy), thin::queries::objectives::scoreable_on(&state, content, sources, player, galaxy), "{}", at(&format!("scoreable_on {player}")));
                let (po, pt) = (old::objectives::Position::new(&state, content, sources, player), thin::queries::objectives::Position::new(&state, content, sources, player));
                let (po, pt) = if let Some(g) = galaxy { (po.with_galaxy(g), pt.with_galaxy(g)) } else { (po, pt) };
                assert_eq!(dbg(&old::objectives::home_planets_not_controlled(&po)), dbg(&thin::queries::objectives::home_planets_not_controlled(&pt)), "{}", at("home_planets_not_controlled"));
                let (o, t) = (old::choice::Observed::new(&state, content, sources, galaxy), thin::observed::Observed::new(&state, content, sources, galaxy));
                assert_eq!(dbg(&o.revealed_objective_progress(player)), dbg(&t.revealed_objective_progress(player)), "{}", at(&format!("revealed_objective_progress {player}")));
                assert_eq!(o.scoreable_public(player), t.scoreable_public(player), "{}", at("scoreable_public"));
                assert_eq!(o.scoreable_secret(player), t.scoreable_secret(player), "{}", at("scoreable_secret"));
                assert_eq!(dbg(&old::choice::held_secret_progress(&state, content, sources, galaxy, player)), dbg(&thin::observed::held_secret_progress(&state, content, sources, galaxy, player)), "{}", at("held_secret_progress"));
            }
        }
        tables += 1;
    }
    assert_eq!(tables, 24);
    assert!(guns > 100 && bombarded > 20 && abilities_held > 30, "guns {guns}, bombardable {bombarded}, abilities held {abilities_held}");
}

#[test]
fn who_can_reach_answers_as_the_legacy_one_does() {
    let content = content();
    let (mut rows, mut boosted) = (0_usize, 0_usize);
    for pos in positions() {
        for destination in systems_of(&pos) {
            let a = old::reach_query::who_can_reach(&pos.state, content, pos.sources, &pos.galaxy, &destination);
            let b = thin::queries::reach_query::who_can_reach(&pos.state, content, pos.sources, &pos.galaxy, &destination);
            assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap(), "{}: who_can_reach {destination}", pos.label);
            rows += a.len();
            boosted += a.iter().flat_map(|o| o.ships.iter()).filter(|s| s.gravity_drive > 0 || s.ionian > 0).count();
        }
    }
    assert!(rows > 300, "only {rows} origins compared ({boosted} with a boost)");
}

#[test]
fn the_secondary_types_are_the_legacy_ones() {
    use ti4_content::StrategyCardKind;
    for kind in [StrategyCardKind::Technology, StrategyCardKind::Diplomacy, StrategyCardKind::Construction, StrategyCardKind::Warfare, StrategyCardKind::Leadership, StrategyCardKind::Trade, StrategyCardKind::Politics, StrategyCardKind::Imperial] {
        assert_eq!(old::secondary_preview::follow_up_subtypes(kind), thin::queries::secondary_preview::follow_up_subtypes(kind), "{kind:?}");
    }
    // The wire shape of every variant: the same JSON from the same fields, and back.
    let choice = old::choice::Choice::new(PlayerId::new("a"), "p", vec![old::choice::ChoiceOption::decline()]);
    let thin_choice: thin::wire::choice::Choice = serde_json::from_value(serde_json::to_value(&choice).unwrap()).unwrap();
    let payment = (3_i64, vec!["jord".to_owned()], 1_i32, 4_i64);
    let skipped = ("subtype".to_owned(), "prompt".to_owned());
    let old_values = [
        old::secondary_preview::SecondaryPreview::Question {
            choice: choice.clone(),
            step: 1,
            payment: Some(old::secondary_preview::PaymentPreview { cost: payment.0, planets: payment.1.clone(), trade_goods: payment.2, worth: payment.3 }),
            skipped: vec![old::secondary_preview::SkippedPrompt { subtype: skipped.0.clone(), prompt: skipped.1.clone() }],
        },
        old::secondary_preview::SecondaryPreview::Question { choice: choice.clone(), step: 0, payment: None, skipped: Vec::new() },
        old::secondary_preview::SecondaryPreview::WouldNotBeAsked { blocker: old::secondary_preview::SecondaryBlocker::CommoditiesFull },
        old::secondary_preview::SecondaryPreview::Complete { skipped: Vec::new(), unused_answers: 0 },
        old::secondary_preview::SecondaryPreview::Complete { skipped: vec![old::secondary_preview::SkippedPrompt { subtype: skipped.0.clone(), prompt: skipped.1.clone() }], unused_answers: 2 },
        old::secondary_preview::SecondaryPreview::Rejected { at: 1, answer: "x".to_owned(), choice: Box::new(choice) },
        old::secondary_preview::SecondaryPreview::Unavailable { detail: "d".to_owned() },
    ];
    for value in old_values {
        let text = serde_json::to_string(&value).unwrap();
        let back: thin::queries::secondary_preview::SecondaryPreview = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text);
    }
    let _ = thin_choice;
    for blocker in ["no_strategy_token", "cannot_afford_influence", "cannot_pay_resources", "nothing_to_research", "no_exhausted_planet", "commodities_full", "no_production", "other"] {
        let a: old::secondary_preview::SecondaryBlocker = serde_json::from_str(&format!("\"{blocker}\"")).unwrap();
        let b: thin::queries::strategy::SecondaryBlocker = serde_json::from_str(&format!("\"{blocker}\"")).unwrap();
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }
    assert_eq!(format!("{:?}", old::strategy::SecondaryResolution::Followed), format!("{:?}", thin::queries::strategy::SecondaryResolution::Followed));
    for relation in ["when", "after"] {
        let a: old::timing::Relation = serde_json::from_str(&format!("\"{relation}\"")).unwrap();
        let b: thin::queries::timing::Relation = serde_json::from_str(&format!("\"{relation}\"")).unwrap();
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }
}

#[cfg(feature = "ts")]
mod ts {
    use ts_rs::{Config, TS};

    use super::{old, thin};

    fn same<L: TS + ?Sized + 'static, T: TS + ?Sized + 'static>() {
        let cfg = Config::new().with_large_int("number").with_import_extension(Some("ts"));
        assert_eq!(L::name(&cfg), T::name(&cfg));
        assert_eq!(L::decl(&cfg), T::decl(&cfg), "the declaration of {}", L::name(&cfg));
        assert_eq!(L::output_path(), T::output_path(), "the export path of {}", L::name(&cfg));
        assert_eq!(L::export_to_string(&cfg).ok(), T::export_to_string(&cfg).ok(), "the export text of {}", L::name(&cfg));
    }

    #[test]
    fn the_ts_types_of_the_surface_agree() {
        same::<old::strategy::SecondaryBlocker, thin::queries::strategy::SecondaryBlocker>();
        same::<old::secondary_preview::SecondaryPreview, thin::queries::secondary_preview::SecondaryPreview>();
        same::<old::secondary_preview::PaymentPreview, thin::queries::secondary_preview::PaymentPreview>();
        same::<old::secondary_preview::SkippedPrompt, thin::queries::secondary_preview::SkippedPrompt>();
        same::<old::reach_query::ReachOrigin, thin::queries::reach_query::ReachOrigin>();
        same::<old::reach_query::ReachShip, thin::queries::reach_query::ReachShip>();
    }
}
