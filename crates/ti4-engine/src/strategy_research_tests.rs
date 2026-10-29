//! Lockstep tests of the Technology paths the core `Research` window carries (package B8c-2): specialty planets,
//! standing waivers, AI Development Algorithm, Inheritance Systems, unit upgrades on the board, the Anti-Intellectual
//! Revolution, Research Agreement and Propagation. Each plays the real ability on a real seated game under the `Dual`
//! strategy backend (the string and the core windows are compared question by question and position by position) and
//! then reads the string position to see that the effect happened at all.

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus};

use crate::choice::{Scripted, Table};
use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

const LABEL_P: &str = "strategy_cards::technology_primary";
const LABEL_S: &str = "strategy_cards::technology_secondary";
const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

/// Play the Technology ability of `a` (`primary` or the secondary) on a seated game under the Dual backend:
/// `(compared, bailed)` of its window and the string position afterwards.
fn play(factions: &[(&str, &str)], setup: impl FnOnce(&mut GameState), answers: &[&str], primary: bool) -> ((u64, u64), GameState) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let mut after = None;
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        let mut state = crate::fixtures::seated_game(factions, POK);
        crate::promissory::deal(&mut state, content(), POK);
        setup(&mut state);
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|x| (*x).to_owned()))));
        if primary {
            crate::strategy_cards::primary(&mut state, content(), POK, None, &mut table, &a(), "pok7technology").unwrap();
        } else {
            crate::strategy_cards::secondary(&mut state, content(), POK, None, &mut table, &a(), "pok7technology").unwrap();
        }
        after = Some(state);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let counts = crate::dual::thread_window_counts(if primary { LABEL_P } else { LABEL_S });
    let _ = take_report();
    (counts, after.expect("played"))
}

fn owns(state: &GameState, who: &str, tech: &str) -> bool {
    state.player(&PlayerId::new(who)).unwrap().technologies.contains(&TechnologyId::new(tech))
}

fn rich(s: &mut GameState) {
    s.player_mut(&a()).unwrap().trade_goods = 6;
}

fn give(s: &mut GameState, who: &str, techs: &[&str]) {
    for t in techs {
        s.player_mut(&PlayerId::new(who)).unwrap().technologies.insert(TechnologyId::new(*t));
    }
}

/// The technologies `a` can research on `state`, those whose prerequisites are met and the rest.
fn researchable(state: &GameState) -> Vec<TechnologyId> {
    crate::technology::researchable(state, content(), POK, &a())
}

/// A planet (not a home planet) with a tech specialty, its system, the specialty colour and a technology of no faction,
/// no unit upgrade, whose only prerequisite is one of that colour.
fn specialty_case() -> (PlanetId, SystemId, &'static str, TechnologyId) {
    let base = crate::fixtures::seated_game(&F3, POK);
    let wanted = |colour: &str| -> Option<TechnologyId> {
        crate::technology::active_aliases(content(), POK).into_iter().find(|t| {
            let need = crate::technology::prerequisites(content(), t);
            need.len() == 1
                && need.get(colour) == Some(&1)
                && !crate::technology::is_unit_upgrade(content(), t)
                && crate::technology::faction_of(content(), t).is_none()
        })
    };
    for (id, planet) in ti4_content::galaxy::all_planets(content(), POK) {
        let (Some(system), false) = (planet.system_id(), planet.is_placed_during_play()) else { continue };
        let planet_id = PlanetId::new(id);
        if id == "jord" {
            continue;
        }
        for colour in crate::technology::COLOURS {
            if crate::planets::tech_specialties_now(&base, content(), POK, &planet_id).iter().any(|s| s.eq_ignore_ascii_case(colour))
                && let Some(tech) = wanted(colour)
            {
                return (planet_id, SystemId::new(system), colour, tech);
            }
        }
    }
    panic!("the corpus has a specialty planet and a technology that needs its colour");
}

#[test]
fn a_specialty_planet_is_exhausted_for_the_prerequisite_in_lockstep() {
    let (planet, system, colour, tech) = specialty_case();
    let control = |s: &mut GameState| s.system_mut(&system).set_control(planet.clone(), a());
    // The primary: the free research of a technology the colours owned do not cover, then the paid one declined.
    let (counts, after) = play(&F3, control, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0), "{colour} {tech}: compared, not declined");
    assert!(owns(&after, "a", tech.as_str()));
    assert!(after.exhausted_planets.contains(&planet), "the specialty planet paid the prerequisite");
    // Owning the colour already: nothing is exhausted.
    let covered = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        let own: Vec<_> = crate::technology::active_aliases(content(), POK)
            .into_iter()
            .filter(|t| crate::technology::colour_type(content(), t) == Some(colour) && crate::technology::prerequisites(content(), t).is_empty())
            .take(1)
            .collect();
        for t in own {
            s.player_mut(&a()).unwrap().technologies.insert(t);
        }
    };
    let (counts, after) = play(&F3, covered, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert!(!after.exhausted_planets.contains(&planet), "the colour is owned: the planet stays ready");
    // Psychoarchaeology frees the specialties of exhausted planets: nothing is exhausted after choosing.
    let psycho = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        give(s, "a", &["pa"]);
    };
    let (counts, after) = play(&F3, psycho, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert!(!after.exhausted_planets.contains(&planet), "Psychoarchaeology keeps the planet ready");
}

#[test]
fn the_secondary_pays_for_a_specialty_research_too() {
    let (planet, system, _, tech) = specialty_case();
    let control = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        s.player_mut(&a()).unwrap().trade_goods = 4;
    };
    // Four resources: the four trade goods pay (the lone plan is taken without a question), the planet is not spent on it.
    let (counts, after) = play(&F3, control, &[tech.as_str()], false);
    assert_eq!(counts.1, 0, "{counts:?}");
    assert_eq!(counts.0, 1);
    assert!(owns(&after, "a", tech.as_str()));
}

#[test]
fn a_standing_waiver_covers_the_prerequisite_before_a_planet_is_spent() {
    let (planet, system, _, tech) = specialty_case();
    // The Prophet's Tears waives one prerequisite: the specialty planet stays ready.
    let tears = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        s.player_mut(&a()).unwrap().relics.push(ti4_model::id::RelicId::new("prophetstears"));
    };
    let (counts, after) = play(&F3, tears, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert!(!after.exhausted_planets.contains(&planet), "the waiver paid, not the planet");
}

/// A technology `a` can research only through its prerequisites being waived or paid, and one unit upgrade.
fn first_unmet(state: &GameState) -> TechnologyId {
    let by_prerequisites = crate::technology::researchable_by_prerequisites(state, content(), POK, &a());
    researchable(state)
        .into_iter()
        .find(|t| !by_prerequisites.contains(t) && !crate::technology::is_unit_upgrade(content(), t))
        .expect("a technology only a waiver opens")
}

#[test]
fn inheritance_systems_pays_two_resources_and_exhausts_the_card_in_lockstep() {
    let owned = |s: &mut GameState| {
        rich(s);
        give(s, "a", &["is"]);
    };
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    owned(&mut probe);
    let tech = first_unmet(&probe);
    let (counts, after) = play(&F3, owned, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert!(after.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("is")), "the card is exhausted");
    assert!(after.player(&a()).unwrap().trade_goods < 6 || !after.exhausted_planets.is_empty(), "two resources were spent");
}

#[test]
fn inheritance_systems_after_the_bill_keeps_the_string_when_the_two_resources_are_gone() {
    // Six to spend on a secondary of four: the technology is offered because Inheritance Systems can pay two, the bill
    // takes four, and two are left only when the planets pay. With exactly four to spend nothing is left for the card,
    // which the string restores the position for. The core window does not carry that rollback.
    let owned = |s: &mut GameState| {
        give(s, "a", &["is"]);
        s.player_mut(&a()).unwrap().trade_goods = 0;
        s.exhausted_planets.insert(PlanetId::new("jord"));
        s.system_mut(&SystemId::new("01")).set_control(PlanetId::new("jord"), a());
    };
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    owned(&mut probe);
    // `jord` pays 4 resources alone once readied; with only it controlled the bill leaves nothing for the card.
    probe.exhausted_planets.remove(&PlanetId::new("jord"));
    probe.player_mut(&a()).unwrap().trade_goods = 4;
    let unmet = first_unmet(&probe);
    let spend_all = |s: &mut GameState| {
        give(s, "a", &["is"]);
        s.player_mut(&a()).unwrap().trade_goods = 4;
        s.exhausted_planets.insert(PlanetId::new("jord"));
    };
    let (counts, _) = play(&F3, spend_all, &[unmet.as_str()], false);
    assert_eq!(counts, (0, 1), "the card cannot pay after the bill: the string's rollback");
}

/// A unit upgrade `a` can research with the green technologies given.
fn researchable_by_prerequisites(state: &GameState) -> Vec<TechnologyId> {
    crate::technology::researchable_by_prerequisites(state, content(), POK, &a())
}

fn infantry_upgrade(extra: &[&str]) -> (impl Fn(&mut GameState) + Clone + use<>, TechnologyId) {
    let extra: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
    let setup = move |s: &mut GameState| {
        rich(s);
        for t in ["nm", "dxa"].iter().map(|t| (*t).to_owned()).chain(extra.clone()) {
            s.player_mut(&a()).unwrap().technologies.insert(TechnologyId::new(t));
        }
    };
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    setup(&mut probe);
    let upgrade = researchable(&probe)
        .into_iter()
        .find(|t| {
            crate::technology::is_unit_upgrade(content(), t)
                && crate::technology::prerequisites(content(), t) == std::collections::BTreeMap::from([("BIOTIC", 2)])
        })
        .expect("an upgrade is researchable with two green");
    (setup, upgrade)
}

#[test]
fn a_unit_upgrade_changes_the_units_on_the_board_in_lockstep() {
    let (setup, upgrade) = infantry_upgrade(&[]);
    let system = SystemId::new("01");
    let with_units = move |s: &mut GameState| {
        setup(s);
        crate::fixtures::put_on_planet(s, &system, &PlanetId::new("jord"), "infantry", &a(), 2);
        crate::fixtures::put(s, &system, "fighter", &a(), 1);
    };
    let (counts, after) = play(&F3, with_units, &[upgrade.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", upgrade.as_str()));
    let better = ti4_content::units::catalogue(content(), POK)
        .values()
        .find(|k| k.required_technology() == Some(upgrade.as_str()))
        .map(|k| k.id().to_owned())
        .expect("the upgrade names its unit");
    let all: Vec<String> = after
        .board
        .values()
        .flat_map(|b| b.units.iter().chain(b.planet_units.values().flatten()))
        .filter(|u| u.owner == a())
        .map(|u| u.type_id.to_string())
        .collect();
    assert!(all.contains(&better), "{better} on the board: {all:?}");
}

#[test]
fn aida_covers_one_prerequisite_of_an_upgrade_and_is_exhausted_only_then() {
    let alone = |s: &mut GameState| {
        rich(s);
        give(s, "a", &["nm", "aida"]);
    };
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    alone(&mut probe);
    let upgrade = researchable(&probe).into_iter().find(|t| crate::technology::is_unit_upgrade(content(), t));
    let Some(upgrade) = upgrade else { panic!("AI Development Algorithm opens an upgrade one green short") };
    let (counts, after) = play(&F3, alone, &[upgrade.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", upgrade.as_str()));
    assert!(after.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("aida")), "spent: nothing else covers it");
    // Two green owned: the upgrade needs no waiver, AIDA stays ready.
    let (setup, upgrade) = infantry_upgrade(&["aida"]);
    let (counts, after) = play(&F3, setup, &[upgrade.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", upgrade.as_str()));
    assert!(!after.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("aida")), "not needed");
}

#[test]
fn the_anti_intellectual_revolution_destroys_the_cheapest_ship_after_a_research() {
    let tech = researchable(&crate::fixtures::seated_game(&F3, POK))[0].clone();
    let setup = |s: &mut GameState| {
        rich(s);
        s.enact_law("revolution", "for");
        let system = SystemId::new("01");
        crate::fixtures::put(s, &system, "cruiser", &a(), 1);
        crate::fixtures::put(s, &system, "destroyer", &a(), 1);
        crate::fixtures::put(s, &system, "fighter", &a(), 2);
    };
    let count = |s: &GameState, kind: &str| s.board.values().flat_map(|b| &b.units).filter(|u| u.owner == a() && u.type_id.as_str() == kind).count();
    let mut before = crate::fixtures::seated_game(&F3, POK);
    setup(&mut before);
    let (destroyers, fighters) = (count(&before, "destroyer"), count(&before, "fighter"));
    let (counts, after) = play(&F3, setup, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert_eq!(count(&after, "destroyer"), destroyers - 1, "the cheapest non-fighter ship was destroyed");
    assert_eq!(count(&after, "fighter"), fighters, "fighters are not taxed");
    // The law elected Against taxes nothing.
    let against = |s: &mut GameState| {
        setup(s);
        s.enact_law("revolution", "against");
    };
    let (counts, after) = play(&F3, against, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert_eq!(count(&after, "destroyer"), destroyers);
}

#[test]
fn research_agreement_gives_the_holder_the_technology_and_the_note_goes_home() {
    let jolnar = [("a", "jolnar"), ("b", "hacan"), ("c", "arborec")];
    let lent = |s: &mut GameState| {
        rich(s);
        // The Sucaban offer is a question the core window does not carry: the agent is spent.
        s.player_mut(&a()).unwrap().leaders.insert(LeaderId::new("jolnaragent"), LeaderStatus::Exhausted);
        s.promissory_notes.insert("ra:jolnar".to_owned(), PlayerId::new("b"));
    };
    let mut probe = crate::fixtures::seated_game(&jolnar, POK);
    lent(&mut probe);
    let tech = researchable(&probe)
        .into_iter()
        .find(|t| crate::technology::faction_of(content(), t).is_none() && crate::technology::prerequisites(content(), t).is_empty())
        .expect("a technology of no faction");
    let (counts, after) = play(&jolnar, lent, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0), "{tech}");
    assert!(owns(&after, "a", tech.as_str()) && owns(&after, "b", tech.as_str()), "both own it");
    assert_eq!(after.promissory_notes.get("ra:jolnar"), Some(&a()), "the note went home");
}

#[test]
fn a_nekro_research_becomes_command_tokens_and_gains_nothing() {
    let nekro = [("a", "nekro"), ("b", "hacan"), ("c", "arborec")];
    let mut probe = crate::fixtures::seated_game(&nekro, POK);
    rich(&mut probe);
    let tech = researchable(&probe)[0].clone();
    let (counts, after) = play(&nekro, rich, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(!owns(&after, "a", tech.as_str()), "Propagation replaces the research");
    assert_eq!(after.faction_marks.get("nekro:propagation:a").map(String::as_str), Some("1"));
}

#[test]
fn a_jolnar_waiver_of_a_prerequisite_is_the_cores_analytical() {
    let jolnar = [("a", "jolnar"), ("b", "hacan"), ("c", "arborec")];
    let spent_agent = |s: &mut GameState| {
        rich(s);
        s.player_mut(&a()).unwrap().leaders.insert(LeaderId::new("jolnaragent"), LeaderStatus::Exhausted);
    };
    let mut probe = crate::fixtures::seated_game(&jolnar, POK);
    spent_agent(&mut probe);
    let tech = researchable_by_prerequisites(&probe)
        .into_iter()
        .find(|t| !crate::technology::prerequisites(content(), t).is_empty() && !crate::technology::is_unit_upgrade(content(), t))
        .expect("a technology Analytical opens");
    let (counts, after) = play(&jolnar, spent_agent, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0), "{tech}");
    assert!(owns(&after, "a", tech.as_str()));
}

#[test]
fn bail_the_questions_of_the_paid_research_stay_string() {
    // Jol-Nar's agent is ready and there is infantry to trade: Doctor Sucaban asks before every paid research.
    let jolnar = [("a", "jolnar"), ("b", "hacan"), ("c", "arborec")];
    let mut probe = crate::fixtures::seated_game(&jolnar, POK);
    rich(&mut probe);
    let tech = researchable(&probe)[0].clone();
    let (counts, _) = play(&jolnar, rich, &[tech.as_str(), "decline", "decline"], true);
    assert_eq!(counts, (0, 1), "Doctor Sucaban's offer is the string's");
    // The Deepwrought commander held by another seat offers the researcher a discount.
    let held = |s: &mut GameState| {
        rich(s);
        s.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(LeaderId::new("deepwroughtcommander"), LeaderStatus::Unlocked);
    };
    let deepwrought = [("a", "sol"), ("b", "deepwrought"), ("c", "arborec")];
    let (counts, _) = play(&deepwrought, held, &[tech.as_str(), "decline", "decline"], true);
    assert_eq!(counts.0, 0, "{counts:?}");
}

/// The unit swaps of the core agree with the string ones for a faction with its own units (Sol's carrier is replaced
/// by its upgrade's own form), so a research changes the right form on the board.
#[test]
fn a_faction_unit_form_is_the_one_the_roster_names() {
    let sol_upgrade = |s: &mut GameState| {
        rich(s);
        give(s, "a", &["nm", "dxa"]);
        let system = SystemId::new("01");
        crate::fixtures::put(s, &system, "sol_carrier", &a(), 1);
    };
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    sol_upgrade(&mut probe);
    let tech = researchable(&probe)[0].clone();
    let (counts, _) = play(&F3, sol_upgrade, &[tech.as_str(), "decline"], true);
    assert_eq!(counts.1, 0, "{counts:?}");
}

#[test]
fn a_research_team_law_on_a_controlled_planet_covers_a_prerequisite_of_its_colour() {
    let (planet, system, colour, tech) = specialty_case();
    let alias = format!("rt_{}", colour.to_ascii_lowercase());
    let law = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        // The law is elected onto the home planet, which `a` controls and keeps ready.
        s.enact_law(&alias, "jord");
    };
    let (counts, after) = play(&F3, law, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", tech.as_str()));
    assert!(!after.exhausted_planets.contains(&planet), "the Research Team covers it: the specialty planet stays ready");
    // On a planet somebody else controls the law waives nothing.
    let other = |s: &mut GameState| {
        s.system_mut(&system).set_control(planet.clone(), a());
        s.enact_law(&alias, "jord");
        s.system_mut(&SystemId::new("01")).set_control(PlanetId::new("jord"), PlayerId::new("b"));
    };
    let (counts, after) = play(&F3, other, &[tech.as_str(), "decline"], true);
    assert!(counts.0 + counts.1 == 1, "{counts:?}");
    if counts.0 == 1 {
        assert!(after.exhausted_planets.contains(&planet), "no waiver: the planet pays");
    }
}

#[test]
fn publicize_weapon_schematics_makes_the_war_sun_free_of_prerequisites() {
    let setup = |s: &mut GameState| {
        rich(s);
        s.enact_law("schematics", "for");
        give(s, "b", &["ws"]);
    };
    let (counts, after) = play(&F3, setup, &["ws", "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", "ws"));
    assert!(after.exhausted_planets.is_empty(), "no prerequisite to pay");
}

/// The planets (not the home planet) that print a specialty of `colour`, with their systems.
fn specialty_planets(colour: &str) -> Vec<(PlanetId, SystemId)> {
    let base = crate::fixtures::seated_game(&F3, POK);
    ti4_content::galaxy::all_planets(content(), POK)
        .into_iter()
        .filter(|(id, planet)| *id != "jord" && planet.system_id().is_some() && !planet.is_placed_during_play())
        .filter(|(id, _)| {
            crate::planets::tech_specialties_now(&base, content(), POK, &PlanetId::new(*id)).iter().any(|s| s.eq_ignore_ascii_case(colour))
        })
        .filter_map(|(id, planet)| Some((PlanetId::new(id), SystemId::new(planet.system_id()?))))
        .collect()
}

/// Technologies of no faction whose prerequisites are exactly `needs` (unit upgrades too when `upgrades`).
fn techs_needing(needs: &[(&'static str, usize)], upgrades: bool) -> Vec<TechnologyId> {
    let wanted: std::collections::BTreeMap<&'static str, usize> = needs.iter().copied().collect();
    crate::technology::active_aliases(content(), POK)
        .into_iter()
        .filter(|t| {
            crate::technology::prerequisites(content(), t) == wanted
                && (upgrades || !crate::technology::is_unit_upgrade(content(), t))
                && crate::technology::faction_of(content(), t).is_none()
        })
        .collect()
}

/// The colours `a` holds at the start (Sol's starting technologies count).
fn held(colour: &str) -> usize {
    let base = crate::fixtures::seated_game(&F3, POK);
    crate::technology::owned_colours(&base, content(), &a()).get(colour).copied().unwrap_or(0)
}

/// Regression (mutation `exhausted_for_colour not counted`): two missing prerequisites of one colour exhaust two
/// specialty planets, not every one the seat controls.
#[test]
fn a_shortage_of_two_exhausts_two_of_three_specialty_planets() {
    for colour in crate::technology::COLOURS {
        let planets = specialty_planets(colour);
        if planets.len() < 3 {
            continue;
        }
        for need in 2..=4 {
            if need < held(colour) + 2 || need > held(colour) + 2 {
                continue;
            }
            for tech in techs_needing(&[(colour, need)], false) {
                let control = |s: &mut GameState| {
                    for (planet, system) in planets.iter().take(3) {
                        s.system_mut(system).set_control(planet.clone(), a());
                    }
                };
                let (counts, after) = play(&F3, control, &[tech.as_str(), "decline"], true);
                if counts != (1, 0) || !owns(&after, "a", tech.as_str()) {
                    continue;
                }
                let spent = planets.iter().take(3).filter(|(p, _)| after.exhausted_planets.contains(p)).count();
                assert_eq!(spent, 2, "{colour} {tech}: exactly the shortage is paid by planets");
                return;
            }
        }
    }
    panic!("the corpus has a technology two colours short and three planets of that colour");
}

/// Regression (mutation `already-chosen planet reused`): a planet that carries two specialties (one printed, one by a
/// research facility) pays one of two colours; the other colour needs a second planet.
#[test]
fn a_planet_chosen_for_one_colour_is_not_chosen_again_for_the_next() {
    use crate::generated::IdStr as _;
    for (i, x) in crate::technology::COLOURS.iter().enumerate() {
        for y in &crate::technology::COLOURS[i + 1..] {
            if held(x) != 0 || held(y) != 0 {
                continue;
            }
            let Some(facility) = ti4_engine_core::generated::Attachment::ALL.into_iter().find(|att| {
                let want = ti4_engine_core::generated::TechType::from_id(y).expect("a colour");
                att.data().specialties == [want]
            }) else {
                continue;
            };
            let (xs, ys) = (specialty_planets(x), specialty_planets(y));
            // The second planet comes later in the seat's controlled order than the first: the first, which carries both
            // specialties, would be met again for the second colour if it were not set aside.
            let after = |(q, qs): &(PlanetId, SystemId), (p, ps): &(PlanetId, SystemId)| (qs.as_str(), q.as_str()) > (ps.as_str(), p.as_str());
            for (p, p_system) in &xs {
                let Some((q, q_system)) = ys.iter().find(|candidate| after(candidate, &(p.clone(), p_system.clone()))) else { continue };
                for tech in techs_needing(&[(x, 1), (y, 1)], true) {
                    let attachment = facility.as_str().to_owned();
                    let (pc, qc, ps, qs) = (p.clone(), q.clone(), p_system.clone(), q_system.clone());
                    let control = move |s: &mut GameState| {
                        s.system_mut(&ps).set_control(pc.clone(), a());
                        s.system_mut(&qs).set_control(qc.clone(), a());
                        s.planet_attachments.entry(pc.clone()).or_default().push(attachment.clone());
                    };
                    let (counts, after) = play(&F3, control, &[tech.as_str(), "decline"], true);
                    if counts != (1, 0) || !owns(&after, "a", tech.as_str()) {
                        continue;
                    }
                    assert!(after.exhausted_planets.contains(p) && after.exhausted_planets.contains(q), "{x}+{y} {tech}: both planets pay, the first is not counted twice");
                    return;
                }
            }
        }
    }
    panic!("the corpus has a two-colour technology and the planets to pay it");
}

/// Regression (mutation `war sun waiver ignored`): with the war sun free of prerequisites a controlled specialty planet
/// of its colour is not exhausted for it.
#[test]
fn a_waived_war_sun_does_not_exhaust_a_specialty_planet() {
    let needs = crate::technology::prerequisites(content(), &TechnologyId::new("ws"));
    let (colour, _) = needs.iter().next().map(|(c, n)| (*c, *n)).expect("the war sun has prerequisites");
    let (planet, system) = specialty_planets(colour).into_iter().next().expect("a planet of its colour");
    let setup = |s: &mut GameState| {
        rich(s);
        s.enact_law("schematics", "for");
        give(s, "b", &["ws"]);
        s.system_mut(&system).set_control(planet.clone(), a());
    };
    let (counts, after) = play(&F3, setup, &["ws", "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", "ws"));
    assert!(!after.exhausted_planets.contains(&planet), "no prerequisite to pay");
}

#[test]
fn bail_specialist_compounds_stays_string() {
    // The breakthrough and a readied specialty planet: Jol-Nar is offered to exhaust the planet instead of paying.
    let jolnar = [("a", "jolnar"), ("b", "hacan"), ("c", "arborec")];
    let (planet, system, _, tech) = specialty_case();
    let setup = |s: &mut GameState| {
        rich(s);
        s.player_mut(&a()).unwrap().leaders.insert(LeaderId::new("jolnaragent"), LeaderStatus::Exhausted);
        s.player_mut(&a()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("jolnarbt"));
        s.system_mut(&system).set_control(planet.clone(), a());
    };
    let (counts, _) = play(&jolnar, setup, &[tech.as_str(), "decline", "decline"], true);
    assert_eq!(counts, (0, 1), "Specialist Compounds asks before the paid research");
}

#[test]
fn bail_the_seats_the_production_window_does_not_follow() {
    let tech = researchable(&crate::fixtures::seated_game(&F3, POK))[0].clone();
    // An own Keleres seat (the agent offer) and a Yssaril at the table (borrowed agents).
    let keleres = [("a", "keleresm"), ("b", "hacan"), ("c", "arborec")];
    let (counts, _) = play(&keleres, rich, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (0, 1), "keleres");
    let yssaril = [("a", "sol"), ("b", "yssaril"), ("c", "arborec")];
    let (counts, _) = play(&yssaril, rich, &[tech.as_str(), "decline"], true);
    assert_eq!(counts, (0, 1), "yssaril");
}

#[test]
fn bail_a_technology_purged_by_ta_zern() {
    let mut probe = crate::fixtures::seated_game(&F3, POK);
    rich(&mut probe);
    let open = researchable(&probe);
    let (first, last) = (open[0].clone(), open[open.len() - 1].clone());
    // Another technology is purged from every deck: the mark has no core home, so the run is the string's.
    let purged = move |s: &mut GameState| {
        rich(s);
        crate::factions::deepwrought::mark_technology_purged(s, &last);
    };
    let (counts, _) = play(&F3, purged, &[first.as_str(), "decline"], true);
    assert_eq!(counts, (0, 1));
}
