//! Technology effect tests, group g4: faction technologies of Xxcha, Yin, Yssaril, Keleres, Nekro,
//! and the Thunder's Edge factions (see mod.rs). Test names carry the row number of
//! `docs/TECH_EFFECT_COVERAGE.md`.

use std::collections::BTreeMap;


use ti4_model::content_types::{DEFAULT, POK};
use ti4_model::id::{PlanetId, SystemId, TechnologyId};
use ti4_model::state::GameState;

use super::{content, give, pid, seated};
use crate::choice::{ChoiceOption, Scripted, Table};

// -- helpers -----------------------------------------------------------------------------------

fn scripted(answers: &[&str]) -> Table {
    Table::with_default(Box::new(Scripted::new(answers.iter().copied())))
}

/// Emit one typed event through a resolver armed as the game arms one.
fn emit(
    state: &mut GameState,
    answers: &[&str],
    kind: &str,
    pairs: &[(&str, serde_json::Value)],
) {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut table = scripted(answers);
    crate::fixtures::with_context(state, DEFAULT, None, &mut table, |ctx| {
        let payload: BTreeMap<String, serde_json::Value> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        let event = ctx.event_sequence.next(kind, payload).expect("event id");
        resolver
            .emit_with_context(ctx, event, |_, _| {})
            .expect("window resolves");
    });
}

/// The component actions `player` is offered, by option id.
fn offered(state: &GameState, player: &str) -> Vec<String> {
    crate::faction_abilities::component_actions(state, content(), &pid(player))
        .into_iter()
        .map(|option| option.id)
        .collect()
}

/// Perform the faction component action `id` for `player`, answering `answers`.
fn perform(state: &mut GameState, player: &str, id: &str, answers: &[&str]) -> bool {
    let mut table = scripted(answers);
    let option = ChoiceOption::labelled(id, "component", id);
    crate::fixtures::with_context(state, POK, None, &mut table, |ctx| {
        crate::faction_abilities::perform_component(ctx, &pid(player), &option)
    })
}

fn units_in(state: &GameState, system: &str, kind: &str) -> usize {
    state
        .board
        .get(&SystemId::new(system))
        .map_or(0, |board| {
            board.units.iter().filter(|u| u.type_id.as_str() == kind).count()
        })
}

fn on_planet(state: &GameState, system: &str, planet: &str, kind: &str) -> usize {
    state
        .board
        .get(&SystemId::new(system))
        .and_then(|board| board.planet_units.get(&PlanetId::new(planet)))
        .map_or(0, |units| units.iter().filter(|u| u.type_id.as_str() == kind).count())
}

/// Fight the combat in `system` through the combat window, armed as the game arms it; returns the
/// `(player, unit)` of every `SHIP_DESTROYED`, in order.
fn fought(mut state: GameState, system: &SystemId, answers: &[&str]) -> Vec<(String, String)> {
    use crate::choice::{Resolving, TimingHandle, Window};
    use crate::timing::{Ability, Relation};
    use std::sync::{Arc, Mutex};
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let log = Arc::clone(&seen);
    resolver.register([Ability::new(
        "probe:SHIP_DESTROYED",
        pid("a"),
        "SHIP_DESTROYED",
        Relation::After,
        Arc::new(move |event, _| {
            log.lock().unwrap().push((
                event.text("player").unwrap_or_default().to_owned(),
                event.text("unit").unwrap_or_default().to_owned(),
            ));
            Ok(())
        }),
    )]);
    let mut sequence = crate::event::EventSequence::new();
    let mut table = scripted(answers);
    let mut dice = crate::dice::Dice::new();
    let mut rng = crate::rng::GameRng::new(5);
    let mut window = crate::combat::CombatWindow::new(&state, content(), DEFAULT, system);
    let mut ctx = Resolving {
        content: content(),
        sources: DEFAULT,
        dice: &mut dice,
        rng: &mut rng,
        table: &mut table,
        timing: Some(TimingHandle {
            resolver: &mut resolver,
            sequence: &mut sequence,
            galaxy: None,
        }),
    };
    window.settle_open(&mut state, &mut ctx).expect("opens");
    while window.outcome().is_none() {
        window.drive(&mut state, &mut ctx).expect("drives");
        if window.outcome().is_some() {
            break;
        }
        let _ = window.take_scoring_occurrence();
        window.settle_open(&mut state, &mut ctx).expect("settles");
    }
    let guard = seen.lock().unwrap();
    guard.clone()
}

// -- Xxcha: Nullification Field ----------------------------------------------------------------

#[test]
fn nf_1_ends_the_activating_players_turn_for_a_token_and_exhausts() {
    let mut state = seated(&[("a", "sol"), ("b", "xxcha")]);
    let system = SystemId::new("19");
    crate::fixtures::put(&mut state, &system, "cruiser", &pid("b"), 1);
    give(&mut state, "b", "nf");
    state.player_mut(&pid("b")).unwrap().strategic_tokens = 2;
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let used = crate::faction_techs::offer_nullification_field(
        &mut state, content(), POK, &mut table, None, &system, &pid("a"),
    );
    assert_eq!(used, Some(pid("b")), "the holder ends a's turn");
    let seat = state.player(&pid("b")).unwrap();
    assert_eq!(seat.strategic_tokens, 1);
    assert!(seat.exhausted_technologies.contains(&TechnologyId::new("nf")));

    // Without the technology nothing is offered.
    let mut bare = seated(&[("a", "sol"), ("b", "xxcha")]);
    crate::fixtures::put(&mut bare, &system, "cruiser", &pid("b"), 1);
    bare.player_mut(&pid("b")).unwrap().technologies.remove(&TechnologyId::new("nf"));
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    assert_eq!(
        crate::faction_techs::offer_nullification_field(
            &mut bare, content(), POK, &mut table, None, &system, &pid("a"),
        ),
        None
    );
}

// -- Yin: Yin Spinner Omega, Impulse Core ------------------------------------------------------

fn produced(player: &str) -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("player", player.into()),
        ("system", "03".into()),
        ("source", "production".into()),
        ("count", 1.into()),
    ]
}

#[test]
fn yso_1_places_two_infantry_after_producing() {
    let mut state = seated(&[("a", "yin"), ("b", "sol")]);
    give(&mut state, "a", "yso");
    let before = on_planet(&state, "03", "darien", "infantry");
    emit(
        &mut state,
        &["technology:yin:yso:UNITS_PRODUCED:after", "03|darien"],
        "UNITS_PRODUCED",
        &produced("a"),
    );
    assert_eq!(on_planet(&state, "03", "darien", "infantry"), before + 2);

    // Without the technology, or when another player produced, nothing is placed.
    let mut bare = seated(&[("a", "yin"), ("b", "sol")]);
    bare.player_mut(&pid("a")).unwrap().technologies.remove(&TechnologyId::new("yso"));
    emit(&mut bare, &[], "UNITS_PRODUCED", &produced("a"));
    assert_eq!(on_planet(&bare, "03", "darien", "infantry"), before);
    emit(&mut state, &[], "UNITS_PRODUCED", &produced("b"));
    assert_eq!(on_planet(&state, "03", "darien", "infantry"), before + 2);
}

#[test]
fn ic_1_sacrifices_a_destroyer_at_combat_start_for_a_hit() {
    let mut state = seated(&[("a", "yin"), ("b", "sol")]);
    let system = SystemId::new("18");
    give(&mut state, "a", "ic");
    crate::fixtures::put(&mut state, &system, "destroyer", &pid("a"), 1);
    crate::fixtures::put(&mut state, &system, "dreadnought", &pid("b"), 1);
    let destroyed = fought(state, &system, &["destroyer"]);
    assert_eq!(
        destroyed.first(),
        Some(&("a".to_owned(), "destroyer".to_owned())),
        "the sacrifice comes before any die: {destroyed:?}"
    );

    // Without the technology the destroyer is not offered up (the script has no answer to spend).
    let mut bare = seated(&[("a", "yin"), ("b", "sol")]);
    crate::fixtures::put(&mut bare, &system, "destroyer", &pid("a"), 1);
    crate::fixtures::put(&mut bare, &system, "dreadnought", &pid("b"), 4);
    let plain = fought(bare, &system, &[]);
    assert!(plain.iter().all(|(player, _)| player == "a"), "{plain:?}");
}

// -- Yssaril: Transparasteel Plating -----------------------------------------------------------

#[test]
fn tp_1_passed_players_cannot_play_action_cards_on_the_owners_turn() {
    let mut state = seated(&[("a", "yssaril"), ("b", "sol")]);
    give(&mut state, "a", "tp");
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid("a"));
    state.player_mut(&pid("b")).unwrap().passed = true;
    assert!(crate::laws::action_cards_forbidden(&state, &pid("b")));
    assert!(!crate::laws::action_cards_forbidden(&state, &pid("a")));
    // Not the owner's turn: allowed.
    state.active = Some(pid("b"));
    assert!(!crate::laws::action_cards_forbidden(&state, &pid("b")));
    // Without the technology: allowed.
    state.active = Some(pid("a"));
    state.player_mut(&pid("a")).unwrap().technologies.remove(&TechnologyId::new("tp"));
    assert!(!crate::laws::action_cards_forbidden(&state, &pid("b")));
}

// -- Keleres: I.I.H.Q. Modernization, Agency Supply Network, Executive Order -------------------

#[test]
fn iihq_1_makes_neighbours_of_everyone_near_mecatol() {
    let hub = crate::fixtures::hub_with_centre("18");
    let mut state = seated(&[("a", "keleresm"), ("b", "sol")]);
    for sys in state.board.keys().cloned().collect::<Vec<_>>() {
        let board = state.system_mut(&sys);
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
    }
    let (a, b) = (pid("a"), pid("b"));
    crate::fixtures::put(&mut state, &SystemId::new(hub.outer[0].as_str()), "carrier", &a, 1);
    crate::fixtures::put(&mut state, &SystemId::new(hub.outer[3].as_str()), "carrier", &b, 1);
    assert!(!crate::transactions::are_neighbours(&state, &hub.galaxy, &a, &b), "no card");
    give(&mut state, "a", "iihq");
    assert!(crate::transactions::are_neighbours(&state, &hub.galaxy, &a, &b));
    assert!(crate::transactions::are_neighbours(&state, &hub.galaxy, &b, &a), "mutual");
    // A player with nothing near Mecatol is not made a neighbour.
    state.system_mut(&SystemId::new(hub.outer[3].as_str())).units.clear();
    let far = crate::fixtures::plain_systems(20).pop().unwrap();
    crate::fixtures::put(&mut state, &SystemId::new(far), "carrier", &b, 1);
    assert!(!crate::transactions::are_neighbours(&state, &hub.galaxy, &a, &b));
}

#[test]
fn iihq_2_gains_custodia_vigilia_and_cannot_lose_it() {
    let mut state = seated(&[("a", "keleresm"), ("b", "sol")]);
    crate::factions::keleres::reconcile(&mut state);
    assert!(!crate::factions::keleres::holds_custodia(&state, &pid("a")), "no card, no planet");
    give(&mut state, "a", "iihq");
    crate::factions::keleres::reconcile(&mut state);
    assert!(crate::factions::keleres::holds_custodia(&state, &pid("a")));
    // Someone else taking control of it is undone.
    let home = state.player(&pid("a")).unwrap().home_system.clone().unwrap();
    state.system_mut(&home).set_control(PlanetId::new("custodiavigilia"), pid("b"));
    crate::factions::keleres::reconcile(&mut state);
    assert!(crate::factions::keleres::holds_custodia(&state, &pid("a")), "cannot be lost");
}

#[test]
fn asn_1_resolves_a_second_production_in_any_system_once_per_action() {
    use crate::factions::keleres::asn_ready;
    let mut state = seated(&[("a", "keleresm"), ("b", "sol")]);
    give(&mut state, "a", "asn");
    state.player_mut(&pid("a")).unwrap().trade_goods = 20;
    let home = state.player(&pid("a")).unwrap().home_system.clone().unwrap();
    let (other, planet) = crate::fixtures::a_placed_planet();
    state.system_mut(&other).set_control(planet.clone(), pid("a"));
    crate::fixtures::put_on_planet(&mut state, &other, &planet, "spacedock", &pid("a"), 1);
    let ships = |s: &GameState, sys: &SystemId| {
        s.system_state(sys)
            .units
            .iter()
            .filter(|u| u.owner == pid("a") && u.type_id.as_str() == "destroyer")
            .count()
    };
    let home_before = ships(&state, &home);
    let target = other.to_string();
    let mut built = false;
    let mut table = Table::default();
    table.seat(
        pid("a"),
        Box::new(Pick(move |choice: &crate::choice::Choice| {
            if choice.prompt.starts_with("Agency Supply Network") {
                return choice.option(&target).cloned();
            }
            if !built && let Some(o) = choice.options.iter().find(|o| o.id.starts_with("build|destroyer")) {
                built = true;
                return Some(o.clone());
            }
            if let Some(o) = choice.option("trade_good") {
                return Some(o.clone());
            }
            choice.options.iter().find(|o| o.is_decline()).cloned()
        })),
    );
    let report = crate::production::resolve(&mut state, content(), DEFAULT, None, &mut table, &pid("a"), &home);
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(ships(&state, &other), 1, "built where ASN pointed, not at the first dock");
    assert_eq!(ships(&state, &home), home_before);
    assert!(!asn_ready(&state, &pid("a")), "once per action");
}

struct Pick<F>(F);
impl<F: FnMut(&crate::choice::Choice) -> Option<ChoiceOption>> crate::choice::Decider for Pick<F> {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
        (self.0)(choice)
            .or_else(|| choice.options.first().cloned())
            .ok_or_else(|| crate::choice::IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            })
    }
}

#[test]
fn executiveorder_1_draws_the_top_or_bottom_agenda_and_exhausts() {
    let mut state = seated(&[("a", "keleresm"), ("b", "sol")]);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid("a"));
    state.agenda_deck = vec!["sanctions".to_owned(), "mutiny".to_owned()];
    assert!(offered(&state, "a").is_empty(), "no technology, no action");
    give(&mut state, "a", "executiveorder");
    let options = offered(&state, "a");
    assert!(options.iter().any(|o| o.ends_with("executiveorder|bottom")), "{options:?}");
    let bottom = options.iter().find(|o| o.ends_with("|bottom")).unwrap().clone();
    assert!(perform(&mut state, "a", &bottom, &[]));
    assert_eq!(state.agenda_deck, ["sanctions"], "the bottom card was drawn");
    assert!(state.player(&pid("a")).unwrap().exhausted_technologies.contains(&TechnologyId::new("executiveorder")));
    assert_eq!(
        crate::factions::keleres::take_executive_order(&mut state),
        Some((pid("a"), "mutiny".to_owned())),
        "the drawn agenda is handed to the vote"
    );
    assert!(offered(&state, "a").is_empty(), "exhausted");
}

// -- Nekro: ???_NULL_REFERENCE_???, ???_ERROR_ERROR_??? ----------------------------------------
// Both are implemented in nekro_units.rs but deliberately unclaimed (operator ruling 2026-10-07:
// an obscure variant), so they are tested at the hook, not through a faction-technology list.

fn ship_destroyed(state: &mut GameState, unit: &str) {
    emit(
        state,
        &[],
        "SHIP_DESTROYED",
        &[
            ("system", "18".into()),
            ("player", "a".into()),
            ("unit", unit.into()),
            ("last", false.into()),
            ("cause", "space_combat".into()),
            ("during_space_combat", true.into()),
        ],
    );
}

fn cruisers_at_home(state: &GameState) -> usize {
    let home = state.player(&pid("a")).unwrap().home_system.clone().unwrap();
    state
        .system_state(&home)
        .units
        .iter()
        .filter(|u| u.owner == pid("a") && u.type_id.as_str() == "cruiser")
        .count()
}

#[test]
fn nekroc4y_1_produces_the_destroyed_ship_type_at_a_home_space_dock() {
    let mut state = seated(&[("a", "nekro"), ("b", "sol")]);
    give(&mut state, "a", "nekroc4y");
    state.player_mut(&pid("a")).unwrap().trade_goods = 10;
    let before = cruisers_at_home(&state);
    ship_destroyed(&mut state, "cruiser");
    assert_eq!(cruisers_at_home(&state), before + 1);

    // Without the card nothing is produced.
    let mut bare = seated(&[("a", "nekro"), ("b", "sol")]);
    bare.player_mut(&pid("a")).unwrap().trade_goods = 10;
    ship_destroyed(&mut bare, "cruiser");
    assert_eq!(cruisers_at_home(&bare), before);
}

fn with_c4r(state: &mut GameState) {
    give(state, "a", "nekroc4r");
}

fn damage_all(state: &mut GameState, who: &str) {
    for board in state.board.values_mut() {
        for unit in board
            .units
            .iter_mut()
            .chain(board.planet_units.values_mut().flatten())
            .filter(|u| u.owner == pid(who))
        {
            unit.sustained_damage = true;
        }
    }
}

fn pds_of(state: &GameState, who: &str) -> usize {
    state
        .board
        .values()
        .flat_map(|board| board.planet_units.values().flatten())
        .filter(|u| u.owner == pid(who) && u.type_id.as_str() == "pds")
        .count()
}

#[test]
fn nekroc4r_1_places_a_pds_on_a_controlled_planet_and_exhausts() {
    let mut state = seated(&[("a", "nekro"), ("b", "sol")]);
    assert!(!offered(&state, "a").iter().any(|o| o.contains("c4r")), "no card");
    with_c4r(&mut state);
    let before = pds_of(&state, "a");
    assert!(perform(&mut state, "a", "faction|nekro|c4r|pds", &[]));
    assert_eq!(pds_of(&state, "a"), before + 1);
    assert!(state.player(&pid("a")).unwrap().exhausted_technologies.contains(&TechnologyId::new("nekroc4r")));
    assert!(!perform(&mut state, "a", "faction|nekro|c4r|pds", &[]), "exhausted");
}

#[test]
fn nekroc4r_2_repairs_every_damaged_unit_of_the_owner_only() {
    let mut state = seated(&[("a", "nekro"), ("b", "sol")]);
    with_c4r(&mut state);
    damage_all(&mut state, "a");
    damage_all(&mut state, "b");
    assert!(offered(&state, "a").contains(&"faction|nekro|c4r|repair".to_owned()));
    assert!(perform(&mut state, "a", "faction|nekro|c4r|repair", &[]));
    let damaged = |who: &str| {
        state
            .board
            .values()
            .flat_map(|b| b.units.iter().chain(b.planet_units.values().flatten()))
            .filter(|u| u.owner == pid(who) && u.sustained_damage)
            .count()
    };
    assert_eq!(damaged("a"), 0);
    assert!(damaged("b") > 0, "another player's damage stays");
}

#[test]
fn nekroc4r_3_discards_an_action_card_to_draw_one() {
    let mut state = seated(&[("a", "nekro"), ("b", "sol")]);
    with_c4r(&mut state);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("sabo1")];
    let deck = state.action_card_deck.len();
    assert!(perform(&mut state, "a", "faction|nekro|c4r|draw", &[]));
    let seat = state.player(&pid("a")).unwrap();
    assert_eq!(seat.action_cards.len(), 1, "discarded 1, drew 1");
    assert_ne!(seat.action_cards[0].as_str(), "sabo1");
    assert_eq!(state.action_card_deck.len(), deck - 1);
}

// -- Last Bastion: Proxima Targeting VI, 4X41C "Helios" V2 ---------------------------------------

fn bastion_arena(owner_of_planet: &str, tech_holder: Option<&str>, seats: &[(&str, &str)]) -> (GameState, SystemId, PlanetId) {
    let mut state = seated(seats);
    let (system, planet) = crate::fixtures::a_placed_planet();
    let board = state.system_mut(&system);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    board.planet_control.insert(planet.clone(), pid(owner_of_planet));
    if let Some(holder) = tech_holder {
        give(&mut state, holder, "proxima");
    }
    (state, system, planet)
}

fn bombard_three_dreadnoughts(state: &mut GameState, system: &SystemId) {
    let mut dice = crate::dice::Dice::from_faces([10, 10, 10]);
    let mut rng = crate::rng::GameRng::new(0);
    crate::invasion::bombardment(
        state, content(), DEFAULT, &mut dice, &mut rng, &mut scripted(&[]), system, &pid("a"),
    )
    .expect("bombards");
}

#[test]
fn proxima_1_cancels_one_bombardment_hit_per_galvanized_unit_present() {
    let seats = [("a", "sol"), ("b", "bastion")];
    let setup = |tech: bool| {
        let (mut state, system, planet) = bastion_arena("b", tech.then_some("b"), &seats);
        crate::fixtures::put(&mut state, &system, "dreadnought", &pid("a"), 3);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &pid("b"), 4);
        state
            .system_mut(&system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("infantry"), pid("b")).galvanized());
        (state, system, planet)
    };
    let (mut state, system, planet) = setup(true);
    bombard_three_dreadnoughts(&mut state, &system);
    assert_eq!(state.system_state(&system).on_planet_of(&planet, &pid("b")).len(), 5 - 2, "3 hits, 1 cancelled");
    let (mut bare, system, planet) = setup(false);
    bombard_three_dreadnoughts(&mut bare, &system);
    assert_eq!(bare.system_state(&system).on_planet_of(&planet, &pid("b")).len(), 5 - 3, "no Proxima: all 3 land");
}

/// Takes the named option when offered, else declines, else the first option.
struct Prefer(&'static str);
impl crate::choice::Decider for Prefer {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
        choice
            .option(self.0)
            .or_else(|| choice.options.iter().find(|o| o.is_decline()))
            .or_else(|| choice.options.first())
            .cloned()
            .ok_or_else(|| crate::choice::IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            })
    }
}

/// The live invasion over `system` armed as the game arms it; returns the dice for their rolls.
fn invade(state: &mut GameState, system: &SystemId, prefer: &'static str, faces: &[u32]) -> crate::dice::Dice {
    use crate::choice::{Resolving, TimingHandle, Window};
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut sequence = crate::event::EventSequence::new();
    let mut table = Table::with_default(Box::new(Prefer(prefer)));
    let mut dice = crate::dice::Dice::from_faces(faces.iter().copied().chain(std::iter::repeat_n(1, 60)));
    let mut rng = crate::rng::GameRng::new(3);
    let mut window = crate::invasion::InvasionWindow::at_commit_step(state, &pid("a"), system);
    let mut ctx = Resolving {
        content: content(),
        sources: DEFAULT,
        dice: &mut dice,
        rng: &mut rng,
        table: &mut table,
        timing: Some(TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
    };
    window.settle(state, &mut ctx);
    for _ in 0..30 {
        let Some(choice) = window.pending_choice(state, content(), DEFAULT) else { break };
        let option = choice.options[0].clone();
        window.resolve(state, &mut ctx, option).expect("resolves");
        window.settle(state, &mut ctx);
    }
    drop(ctx);
    dice
}

#[test]
fn proxima_2_bombards_at_ground_combat_round_start_and_rolls_back() {
    const PROXIMA_ID: &str = "technology:bastion:proxima:GROUND_COMBAT_ROUND_BEGAN:after";
    let setup = |tech: bool| {
        let (mut state, system, planet) = bastion_arena("b", tech.then_some("a"), &[("a", "bastion"), ("b", "sol")]);
        crate::fixtures::put(&mut state, &system, "infantry", &pid("a"), 3);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &pid("b"), 2);
        (state, system, planet)
    };
    let (mut state, system, planet) = setup(true);
    let dice = invade(&mut state, &system, PROXIMA_ID, &[8, 8, 1, 8, 1, 1]);
    let rolls: Vec<_> = dice.history().iter().filter(|r| r.reason == "proxima targeting").collect();
    assert_eq!(rolls.len(), 2, "BOMBARDMENT 8 (x3) and the roll back, each three dice");
    assert_eq!(rolls[0].faces.len(), 3);
    assert_eq!(rolls[1].faces.len(), 3);
    assert!(state.system_state(&system).on_planet_of(&planet, &pid("b")).is_empty(), "both defenders fell");

    let (mut bare, system, _) = setup(false);
    let dice = invade(&mut bare, &system, PROXIMA_ID, &[10; 30]);
    assert!(dice.history().iter().all(|r| r.reason != "proxima targeting"), "no technology, no bombardment");
}

fn helios_planet() -> (GameState, SystemId, PlanetId) {
    let mut state = seated(&[("a", "bastion"), ("b", "sol")]);
    let (system, planet) = crate::fixtures::a_placed_planet();
    let board = state.system_mut(&system);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    state.system_mut(&system).set_control(planet.clone(), pid("a"));
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "bastion_spacedock", &pid("a"), 1);
    (state, system, planet)
}

fn resources_now(state: &GameState, planet: &PlanetId) -> i64 {
    crate::production::planet_value_now(state, content(), DEFAULT, planet, crate::production::Spend::Resources)
}

fn upgrade_helios(state: &mut GameState, system: &SystemId, planet: &PlanetId) {
    give(state, "a", "helios2");
    crate::technology::apply_unit_upgrades(state, content(), DEFAULT, &pid("a"));
    assert!(
        state.system_state(system).on_planet(planet).iter().any(|u| u.type_id.as_str() == "bastion_spacedock2"),
        "the dock is upgraded in place"
    );
}

#[test]
fn helios2_1_production_is_four_more_than_the_planets_resources() {
    let (mut state, system, planet) = helios_planet();
    let v1 = crate::production::capacity(&state, content(), DEFAULT, &pid("a"), &system);
    assert_eq!(v1, resources_now(&state, &planet) + 2, "V1: 2 more than the resource value");
    upgrade_helios(&mut state, &system, &planet);
    assert_eq!(
        crate::production::capacity(&state, content(), DEFAULT, &pid("a"), &system),
        resources_now(&state, &planet) + 4
    );
}

#[test]
fn helios2_2_raises_the_planets_resource_value_by_two() {
    let (mut state, system, planet) = helios_planet();
    let printed = crate::production::planet_value(content(), DEFAULT, &planet, crate::production::Spend::Resources);
    assert_eq!(resources_now(&state, &planet), printed + 1, "V1 adds 1");
    upgrade_helios(&mut state, &system, &planet);
    assert_eq!(resources_now(&state, &planet), printed + 2);
    assert_eq!(crate::factions::bastion_units::resource_bonus(&state, &PlanetId::new("nowhere")), 0);
}

#[test]
fn helios2_3_three_fighters_in_the_system_do_not_count_against_capacity() {
    let (mut state, system, planet) = helios_planet();
    let excess = |s: &GameState| crate::fleet::standing(s, content(), DEFAULT, &pid("a"), &system, None).capacity_excess;
    crate::fixtures::put(&mut state, &system, "fighter", &pid("a"), 3);
    assert_eq!(excess(&state), 0, "3 fighters are supported");
    crate::fixtures::put(&mut state, &system, "fighter", &pid("a"), 1);
    assert_eq!(excess(&state), 1, "the fourth is not");
    upgrade_helios(&mut state, &system, &planet);
    assert_eq!(excess(&state), 1, "V2 still frees exactly three");
}
