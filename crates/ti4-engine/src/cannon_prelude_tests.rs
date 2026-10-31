//! The space cannon offense of the aftermath prelude as a core window (package D1b2, A2b-10b), beside the string
//! `AftermathWindow::cannon_prelude`: positions with PDS fire at an arriving fleet, two gunners, Graviton Laser System, the
//! active seat's own guns, the feat and the documented bails. Every test asserts the core half ran (the window was compared).

use super::{AftermathWindow, EventScoreLimit, FeatOccurrence};
use crate::choice::{Resolving, Table};
use crate::dice::Dice;
use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::rng::GameRng;
use ti4_content::ContentStore;
use ti4_model::id::{PlayerId, PlanetId, SystemId};
use ti4_model::state::GameState;

const LABEL: &str = "combat::space_cannon_offense";

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}

/// What one run of the prelude left.
struct Ran {
    state: GameState,
    result: Result<Option<(FeatOccurrence, EventScoreLimit)>, super::GameError>,
    dice: usize,
}

/// A position: `seats` seated, `a` active at the system of a placed planet that `b` controls (`planet` is passed to `setup`).
fn position(seed: u64, seats: &[(&str, &str)], setup: impl Fn(&mut GameState, &SystemId, &PlanetId)) -> (GameState, SystemId) {
    let mut state = crate::fixtures::seated_game(seats, ti4_model::content_types::DEFAULT);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    state.rng_seed = seed;
    let (system, planet) = crate::fixtures::a_placed_planet();
    state.active = Some(a());
    state.active_system = Some(system.clone());
    state.system_mut(&system).set_control(planet.clone(), b());
    setup(&mut state, &system, &planet);
    (state, system)
}

fn run(mut state: GameState, system: &SystemId, seed: u64) -> Ran {
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let mut ctx = Resolving {
        content: ContentStore::embedded(),
        sources: ti4_model::content_types::DEFAULT,
        dice: &mut dice,
        rng: &mut rng,
        table: &mut table,
        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
    };
    let result = AftermathWindow::cannon_prelude(&mut state, &mut ctx, &a(), system, None);
    let rolled = dice.history().iter().filter(|r| r.reason == "space cannon").count();
    Ran { state, result, dice: rolled }
}

/// Runs `seeds` positions on `driver=dual`; returns the runs and the report (taken).
fn lockstep(seeds: std::ops::Range<u64>, seats: &[(&str, &str)], setup: impl Fn(&mut GameState, &SystemId, &PlanetId) + Copy) -> (Vec<Ran>, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut ran = Vec::new();
    for seed in seeds {
        let (state, system) = position(seed, seats, setup);
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || ran.push(run(state, &system, seed)));
    }
    let r = report();
    let _ = take_report();
    (ran, r)
}

fn compared(r: &crate::dual::DualReport) -> u64 {
    r.windows.get(LABEL).map_or(0, |w| w.compared)
}

fn pds_at_b(s: &mut GameState, system: &SystemId, planet: &PlanetId) {
    crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 2);
    crate::fixtures::put(s, system, "cruiser", &a(), 3);
    crate::fixtures::put(s, system, "fighter", &a(), 2);
}

#[test]
fn a_pds_fires_at_the_arriving_fleet_in_the_core_window_in_lockstep() {
    let (ran, r) = lockstep(0..14, &[("a", "sol"), ("b", "hacan")], pds_at_b);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 10, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().all(|x| x.dice > 0), "the guns rolled");
    let lost = ran.iter().filter(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() < 5).count();
    assert!(lost > 0, "some hit landed");
}

fn ran_system(x: &Ran) -> String {
    x.state.active_system.as_ref().map(ToString::to_string).unwrap_or_default()
}

#[test]
fn a_dreadnought_is_offered_its_sustain_and_graviton_binds_the_loss_in_lockstep() {
    let gls = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 4);
        crate::fixtures::put(s, system, "dreadnought", &a(), 2);
        crate::fixtures::put(s, system, "fighter", &a(), 3);
        s.player_mut(&b()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gls"));
    };
    let (ran, r) = lockstep(0..14, &[("a", "sol"), ("b", "hacan")], gls);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 10, "the core window ran: {:?}", r.windows);
    let bound = ran.iter().filter(|x| x.state.player(&b()).unwrap().exhausted_technologies.contains(&ti4_model::id::TechnologyId::new("gls"))).count();
    assert!(bound > 0, "Graviton Laser System was exhausted to bind a loss");
}

/// W12: a game below `RULES_FACTION_EFFECTS` is carried (the guard "rules version below the faction effects" is gone): the victim's
/// sustain is answered by the same window, and an unlocked Letnev commander is paid his trade good without a question.
#[test]
fn a_game_below_the_faction_effects_fires_the_guns_in_the_core_window_and_pays_the_commander_unasked() {
    let old = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        s.rules_version = ti4_model::state::RULES_FACTION_EFFECTS - 1;
        crate::fixtures::put_on_planet(s, system, planet, "pds", &a(), 4);
        s.system_mut(system).set_control(planet.clone(), a());
        crate::fixtures::put(s, system, "dreadnought", &b(), 3);
        s.player_mut(&b()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("letnevcommander"), ti4_model::state::LeaderStatus::Unlocked);
    };
    let (ran, r) = lockstep(0..20, &[("a", "sol"), ("b", "letnev")], old);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 15, "the core window ran: {:?}", r.windows);
    let before = {
        let (s, _) = position(0, &[("a", "sol"), ("b", "letnev")], old);
        s.player(&b()).unwrap().trade_goods
    };
    assert!(ran.iter().any(|x| x.state.player(&b()).unwrap().trade_goods > before), "a sustain paid the commander's trade good in some run");
}

#[test]
fn the_active_seats_own_guns_fire_at_the_ships_of_the_seat_it_attacks_in_lockstep() {
    let own = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        s.system_mut(system).set_control(planet.clone(), a());
        crate::fixtures::put_on_planet(s, system, planet, "pds", &a(), 2);
        crate::fixtures::put(s, system, "cruiser", &b(), 2);
        crate::fixtures::put(s, system, "fighter", &b(), 2);
    };
    let (ran, r) = lockstep(0..12, &[("a", "sol"), ("b", "hacan")], own);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 8, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().any(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() < 4), "b lost a ship to a's guns");
}

#[test]
fn two_gunners_roll_in_seat_order_and_both_victims_absorb_in_lockstep() {
    let two = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 3);
        crate::fixtures::put_on_planet(s, system, planet, "pds", &PlayerId::new("c"), 3);
        crate::fixtures::put(s, system, "cruiser", &a(), 6);
    };
    let (ran, r) = lockstep(0..30, &[("a", "sol"), ("b", "hacan"), ("c", "letnev")], two);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 25, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().any(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() <= 3), "both gunners' hits landed in some run");
}

#[test]
fn the_last_non_fighter_ship_falling_to_the_guns_scores_the_feat_in_lockstep() {
    let one = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 3);
        crate::fixtures::put_on_planet(s, system, planet, "pds", &PlayerId::new("c"), 3);
        crate::fixtures::put(s, system, "cruiser", &a(), 1);
    };
    let (ran, r) = lockstep(0..20, &[("a", "sol"), ("b", "hacan"), ("c", "letnev")], one);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 15, "the core window ran: {:?}", r.windows);
    let scored = ran.iter().filter(|x| matches!(x.result, Ok(Some(_)))).count();
    assert!(scored > 0, "the last non-fighter ship fell to the guns of the others in some run");
}

/// Graviton Laser System in every shape of target fleet: it binds the loss only when it can change something (a fleet of fighters
/// and non-fighters), is spent when it does, and is not used when exhausted already.
#[test]
fn graviton_is_used_only_where_it_changes_the_loss_in_lockstep() {
    let gls = |fleet: &'static [(&'static str, usize)], exhausted: bool| {
        move |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
            crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 4);
            for (kind, n) in fleet {
                crate::fixtures::put(s, system, kind, &a(), *n);
            }
            let tech = ti4_model::id::TechnologyId::new("gls");
            let seat = s.player_mut(&b()).unwrap();
            seat.technologies.insert(tech.clone());
            if exhausted {
                seat.exhausted_technologies.insert(tech);
            }
        }
    };
    let used = |fleet: &'static [(&'static str, usize)], exhausted: bool| {
        let (ran, r) = lockstep(0..40, &[("a", "sol"), ("b", "hacan")], gls(fleet, exhausted));
        assert_eq!(r.total_differences(), 0, "{fleet:?} {exhausted}: {:?}", r.differences.first());
        assert!(compared(&r) >= 25, "{:?}", r.windows);
        ran.iter().filter(|x| x.state.player(&b()).unwrap().exhausted_technologies.contains(&ti4_model::id::TechnologyId::new("gls"))).count()
    };
    assert!(used(&[("dreadnought", 2), ("fighter", 3)], false) > 0, "a mixed fleet: bound and spent");
    assert_eq!(used(&[("fighter", 4)], false), 0, "only fighters: nothing to bind");
    assert_eq!(used(&[("dreadnought", 2), ("cruiser", 2)], false), 0, "no fighters: every ship is bound anyway");
    assert_eq!(used(&[("dreadnought", 2), ("fighter", 3)], true), 40, "exhausted already: stays exhausted, not used again");
}

/// A planet attachment that gives its planet SPACE CANNON (Titans' Geoform) fires as a unit would.
#[test]
fn an_attachment_with_space_cannon_fires_at_the_arriving_fleet_in_lockstep() {
    let geoform = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        let _ = system;
        s.planet_attachments.entry(planet.clone()).or_default().push("titanshero".to_owned());
        crate::fixtures::put(s, system, "cruiser", &a(), 4);
    };
    let (ran, r) = lockstep(0..20, &[("a", "sol"), ("b", "hacan")], geoform);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 15, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().all(|x| x.dice == 1), "one roll of three dice by the attachment");
    assert!(ran.iter().any(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() < 4), "some hit landed");
}

#[test]
fn a_heart_of_ixth_or_a_jolnar_commander_keeps_the_offense_on_the_string() {
    let heart = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        pds_at_b(s, system, planet);
        s.player_mut(&b()).unwrap().relics.push(ti4_model::id::RelicId::new("heartofixth"));
    };
    let (_, r) = lockstep(0..3, &[("a", "sol"), ("b", "hacan")], heart);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert_eq!(compared(&r), 0, "the window did not run: {:?}", r.windows);
    // The bail is decided before the string runs (`cannon_prelude` always goes through the dual on `driver=dual`).
    let (who, sys) = (a(), SystemId::new("18"));
    let (state, _) = position(0, &[("a", "sol"), ("b", "hacan")], heart);
    let args = crate::cannon_dual::Args { active: &who, system: &sys, announce: true, reproducible: true };
    assert!(crate::cannon_dual::unported(&state, &args).is_some_and(|why| why.contains("reroll question")));
}

#[test]
fn irreproducible_dice_and_dropped_factions_are_documented_bails() {
    let (state, _) = position(0, &[("a", "sol"), ("b", "hacan")], pds_at_b);
    let (system, who) = (SystemId::new("18"), a());
    let args = |reproducible| crate::cannon_dual::Args { active: &who, system: &system, announce: true, reproducible };
    assert_eq!(crate::cannon_dual::unported(&state, &args(true)), None, "the plain position is carried");
    assert!(crate::cannon_dual::unported(&state, &args(false)).is_some_and(|why| why.contains("irreproducible")));
    let (dropped, _) = position(0, &[("a", "sol"), ("b", "obsidian")], pds_at_b);
    assert!(crate::cannon_dual::unported(&dropped, &args(true)).is_some_and(|why| why.contains("dropped")));
}

const HEAD: &str = "aftermath::head";

fn open(mut state: GameState, system: &SystemId, seed: u64, skip: bool) -> (GameState, bool) {
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let mut ctx = Resolving {
        content: ContentStore::embedded(),
        sources: ti4_model::content_types::DEFAULT,
        dice: &mut dice,
        rng: &mut rng,
        table: &mut table,
        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
    };
    let notes = crate::combat::note_holdings(&state);
    let opened = if skip {
        AftermathWindow::at_commit_step(&mut state, &mut ctx, &a(), system, None, notes)
    } else {
        AftermathWindow::new(&mut state, &mut ctx, &a(), system, None, notes)
    };
    let ok = opened.is_ok();
    (state, ok)
}

fn head_lockstep(seeds: std::ops::Range<u64>, skip: bool, setup: impl Fn(&mut GameState, &SystemId, &PlanetId) + Copy) -> (Vec<GameState>, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut states = Vec::new();
    for seed in seeds {
        let (state, system) = position(seed, &[("a", "sol"), ("b", "hacan")], setup);
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            let (after, ok) = open(state, &system, seed, skip);
            assert!(ok, "the aftermath opened");
            states.push(after);
        });
    }
    let r = report();
    let _ = take_report();
    (states, r)
}

#[test]
fn the_head_of_an_aftermath_runs_as_a_core_window_with_the_fights_opening_writes_in_lockstep() {
    let two_sides = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        pds_at_b(s, system, planet);
        // b has ships too: the head opens a space combat; a's war sun is one of its capital ships.
        crate::fixtures::put(s, system, "cruiser", &b(), 2);
        crate::fixtures::put(s, system, "warsun", &a(), 1);
    };
    let (states, r) = head_lockstep(0..12, false, two_sides);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let head = r.windows.get(HEAD).map_or(0, |w| w.compared);
    assert!(head >= 8, "the head ran in the core: {:?}", r.windows);
    assert!(states.iter().any(|s| s.active_space_combat.is_some() || s.combat_round_seq > 0), "a fight opened in some run");
}

#[test]
fn an_action_that_skipped_to_the_commit_step_has_no_cannon_and_no_fight_in_the_core_head() {
    let (states, r) = head_lockstep(0..4, true, pds_at_b);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(HEAD).map_or(0, |w| w.compared) >= 3, "the head ran in the core: {:?}", r.windows);
    assert!(states.iter().all(|s| s.active_space_combat.is_none()), "no fight opens at the commit step");
}

#[test]
fn a_head_with_a_heart_of_ixth_keeps_the_string_and_says_why() {
    let heart = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        pds_at_b(s, system, planet);
        s.player_mut(&b()).unwrap().relics.push(ti4_model::id::RelicId::new("heartofixth"));
    };
    let (_, r) = head_lockstep(0..2, false, heart);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert_eq!(r.windows.get(HEAD).map_or(0, |w| w.compared), 0, "{:?}", r.windows);
    assert!(r.windows.iter().any(|(k, w)| k.starts_with(HEAD) && k.contains("[bail: a reroll question") && w.bailed > 0), "{:?}", r.windows);
}

const ALL: &str = "fleet::enforce_everywhere";

fn sweep(seed: u64, setup: impl Fn(&mut GameState, &SystemId)) -> (GameState, usize, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    state.rng_seed = seed;
    let (system, _) = crate::fixtures::a_placed_planet();
    setup(&mut state, &system);
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let removed = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        crate::fleet::enforce_everywhere(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, None, &mut table).unwrap()
    });
    let r = report();
    let _ = take_report();
    (state, removed, r)
}

#[test]
fn the_table_wide_enforcement_runs_as_one_core_window_and_removes_what_the_string_removes() {
    let (state, removed, r) = sweep(1, |s, system| {
        // Fighters with no carrier are over capacity; a fleet of cruisers is over the supply of a table with no tokens spare.
        crate::fixtures::put(s, system, "fighter", &a(), 9);
        crate::fixtures::put(s, system, "cruiser", &a(), 14);
        crate::fixtures::put(s, system, "fighter", &b(), 6);
        s.player_mut(&a()).unwrap().fleet_tokens = 3;
    });
    assert!(removed > 0, "something was over");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(ALL).map_or(0, |w| w.compared) >= 1, "the core window ran: {:?}", r.windows);
    assert!(state.system_state(&crate::fixtures::a_placed_planet().0).units.len() < 29);
}

#[test]
fn a_table_inside_its_limits_asks_nothing_and_the_core_window_agrees() {
    let (_, removed, r) = sweep(2, |s, system| crate::fixtures::put(s, system, "cruiser", &a(), 2));
    assert_eq!(removed, 0);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(ALL).map_or(0, |w| w.compared) >= 1, "{:?}", r.windows);
}

const MINISTER: &str = "turn::minister_of_war";

/// Declines every offer (the first option that is a decline), else takes the first option.
struct Declines;
impl crate::choice::Decider for Declines {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        choice
            .options
            .iter()
            .find(|o| o.is_decline())
            .or_else(|| choice.options.first())
            .cloned()
            .ok_or_else(|| crate::choice::IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

fn minister(decider: Box<dyn crate::choice::Decider>, with_token: bool) -> (super::Game<'static>, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    state.laws.insert("minister_war".to_owned(), "a".to_owned());
    let (system, _) = crate::fixtures::a_placed_planet();
    if with_token {
        state.system_mut(&system).command_tokens.insert(a());
    }
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(a());
    let mut game = super::Game::with_table(state, ContentStore::embedded(), Table::with_default(decider));
    with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || game.minister_of_war());
    let r = report();
    let _ = take_report();
    (game, r)
}

#[test]
fn minister_of_war_discards_for_a_token_and_an_additional_action_in_both_halves() {
    let (game, r) = minister(Box::new(crate::choice::FirstOption), true);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(MINISTER).map_or(0, |w| w.compared) >= 1, "the core window ran: {:?}", r.windows);
    assert!(game.state.transient_flags.has(ti4_model::state::TransientFlags::ADDITIONAL_ACTION), "the turn is retained");
    assert!(!game.state.laws.contains_key("minister_war"), "the law was discarded");
    let (system, _) = crate::fixtures::a_placed_planet();
    assert!(!game.state.system_state(&system).command_tokens.contains(&a()), "the token left the board");
}

#[test]
fn minister_of_war_declined_changes_nothing_and_without_a_token_nothing_is_asked() {
    let (game, r) = minister(Box::new(Declines), true);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(MINISTER).map_or(0, |w| w.compared) >= 1, "{:?}", r.windows);
    assert!(game.state.laws.contains_key("minister_war") && !game.state.transient_flags.has(ti4_model::state::TransientFlags::ADDITIONAL_ACTION));
    let (game, r) = minister(Box::new(crate::choice::FirstOption), false);
    assert_eq!(r.total_differences(), 0);
    assert!(game.state.laws.contains_key("minister_war") && !game.state.transient_flags.has(ti4_model::state::TransientFlags::ADDITIONAL_ACTION), "22.3: no token on the board, no offer: {:?}", r.windows);
}

/// Every system that is an anomaly (a nebula, a supernova, an asteroid field, a rift or a scar).
fn anomalies() -> Vec<SystemId> {
    let content = ContentStore::embedded();
    ti4_content::galaxy::all_systems(content, ti4_model::content_types::DEFAULT)
        .iter()
        .filter(|(_, s)| s.is_anomaly())
        .map(|(id, _)| SystemId::new(*id))
        .collect()
}

/// A system that is no anomaly, holds no unit at the start of a game and is nobody's home.
fn a_plain_system() -> SystemId {
    let content = ContentStore::embedded();
    let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    let homes: Vec<SystemId> = state.players.iter().filter_map(|p| p.home_system.clone()).collect();
    let found = ti4_content::galaxy::all_systems(content, ti4_model::content_types::DEFAULT)
        .iter()
        .map(|(id, s)| (SystemId::new(*id), s))
        .find(|(id, s)| {
            !s.is_anomaly()
                && !homes.contains(id)
                && state.system_state(id).units.is_empty()
                && state.system_state(id).planet_units.values().all(Vec::is_empty)
        })
        .map(|(id, _)| id)
        .expect("the corpus has a quiet plain system");
    found
}

/// `a` and `b` fight in `system`; `setup` places the fleets (and may seat facts such as a home system) before the snapshot, `lose`
/// then takes what the loser lost. Returns the state after the feats were noted, whether any was, and the report.
fn feats_run(
    system: &SystemId,
    winner: Option<PlayerId>,
    rounds: u32,
    setup: impl Fn(&mut GameState, &SystemId),
    lose: impl Fn(&mut GameState, &SystemId),
) -> (GameState, bool, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    setup(&mut state, system);
    let content = ContentStore::embedded();
    let before = crate::combat::before_combat(&state, content, ti4_model::content_types::DEFAULT, system);
    lose(&mut state, system);
    let outcome = crate::combat::CombatOutcome { winner, rounds };
    let noted = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        crate::combat::note_combat_event_feats(&mut state, content, ti4_model::content_types::DEFAULT, system, &before, &outcome, ti4_model::state::FeatOccurrence(7))
    });
    let r = report();
    let _ = take_report();
    (state, noted, r)
}

fn feats_of(state: &GameState, who: &PlayerId) -> Vec<ti4_model::state::Feat> {
    state.player(who).unwrap().event_feats.iter().map(|(f, _)| *f).collect()
}

/// The standard fight: a has a war sun, a flagship, two cruisers and a dreadnought; b a war sun and a cruiser, and loses them.
fn standard(s: &mut GameState, system: &SystemId) {
    crate::fixtures::put(s, system, "warsun", &a(), 1);
    crate::fixtures::put(s, system, "sol_flagship", &a(), 1);
    crate::fixtures::put(s, system, "cruiser", &a(), 2);
    crate::fixtures::put(s, system, "dreadnought", &a(), 1);
    crate::fixtures::put(s, system, "warsun", &b(), 1);
    crate::fixtures::put(s, system, "cruiser", &b(), 1);
}

fn b_loses_everything(s: &mut GameState, system: &SystemId) {
    s.system_mut(system).units.retain(|u| u.owner != b());
}

#[test]
fn the_feats_of_a_won_fight_in_an_anomaly_at_a_rival_home_are_recorded_by_the_core_as_by_the_string() {
    let system = anomalies().remove(0);
    let (state, noted, r) = feats_run(
        &system,
        Some(a()),
        3,
        |s, system| {
            standard(s, system);
            s.player_mut(&b()).unwrap().home_system = Some(system.clone());
            s.player_mut(&b()).unwrap().victory_points = 4;
        },
        b_loses_everything,
    );
    assert!(noted);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.calls) >= 1, "the write was compared: {:?}", r.areas);
    let feats = feats_of(&state, &a());
    for feat in [
        ti4_model::state::Feat::DestroyedACapitalShip,
        ti4_model::state::Feat::HeldThreeShipsAfterASpaceCombat,
        ti4_model::state::Feat::WonInAnAnomaly,
        ti4_model::state::Feat::WonInARivalHome,
        ti4_model::state::Feat::WonBesideASurvivingFlagship,
        ti4_model::state::Feat::WonAgainstThePointsLeader,
    ] {
        assert!(feats.contains(&feat), "{feat:?} in {feats:?}");
    }
}

#[test]
fn every_kind_of_anomaly_counts_and_a_plain_system_does_not() {
    let all = anomalies();
    assert!(all.len() >= 5, "the corpus has several kinds of anomaly");
    for system in &all {
        let (state, _, r) = feats_run(system, Some(a()), 2, standard, b_loses_everything);
        assert_eq!(r.total_differences(), 0, "{system}: {:?}", r.differences.first());
        assert!(feats_of(&state, &a()).contains(&ti4_model::state::Feat::WonInAnAnomaly), "{system}");
    }
    let (state, _, r) = feats_run(&a_plain_system(), Some(a()), 2, standard, b_loses_everything);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(!feats_of(&state, &a()).contains(&ti4_model::state::Feat::WonInAnAnomaly));
}

#[test]
fn a_win_at_the_winners_own_home_is_no_rival_home_and_without_a_flagship_no_flagship_feat() {
    let system = a_plain_system();
    let (state, _, r) = feats_run(
        &system,
        Some(a()),
        2,
        |s, system| {
            // a's war sun is a capital ship but not a flagship.
            crate::fixtures::put(s, system, "warsun", &a(), 1);
            crate::fixtures::put(s, system, "cruiser", &b(), 1);
            s.player_mut(&a()).unwrap().home_system = Some(system.clone());
        },
        b_loses_everything,
    );
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let feats = feats_of(&state, &a());
    assert!(!feats.contains(&ti4_model::state::Feat::WonInARivalHome), "{feats:?}");
    assert!(!feats.contains(&ti4_model::state::Feat::WonBesideASurvivingFlagship), "{feats:?}");
}

#[test]
fn three_non_fighter_ships_are_the_threshold_of_holding_a_fleet() {
    let system = a_plain_system();
    for (ships, held) in [(2_usize, false), (3, true)] {
        let (state, _, r) = feats_run(
            &system,
            None,
            2,
            |s, system| {
                crate::fixtures::put(s, system, "cruiser", &a(), ships);
                crate::fixtures::put(s, system, "fighter", &a(), 4);
                crate::fixtures::put(s, system, "cruiser", &b(), 1);
            },
            b_loses_everything,
        );
        assert_eq!(r.total_differences(), 0, "{ships}: {:?}", r.differences.first());
        assert_eq!(feats_of(&state, &a()).contains(&ti4_model::state::Feat::HeldThreeShipsAfterASpaceCombat), held, "{ships} cruisers and fighters");
    }
}

#[test]
fn a_capital_ship_lost_is_counted_against_that_sides_own_fleet() {
    // a keeps its one war sun; b had two and lost one: only a scores the destroyed capital ship.
    let system = a_plain_system();
    let (state, _, r) = feats_run(
        &system,
        None,
        2,
        |s, system| {
            crate::fixtures::put(s, system, "warsun", &a(), 1);
            crate::fixtures::put(s, system, "warsun", &b(), 2);
        },
        |s, system| {
            let mut gone = false;
            s.system_mut(system).units.retain(|u| {
                let lose = !gone && u.owner == b();
                gone |= lose;
                !lose
            });
        },
    );
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(feats_of(&state, &a()).contains(&ti4_model::state::Feat::DestroyedACapitalShip));
    assert!(!feats_of(&state, &b()).contains(&ti4_model::state::Feat::DestroyedACapitalShip));
}

#[test]
fn a_draw_scores_only_the_feats_that_do_not_ask_for_a_winner_and_a_fight_of_no_rounds_none() {
    let system = anomalies().remove(0);
    let (state, noted, r) = feats_run(&system, None, 2, standard, b_loses_everything);
    assert!(noted);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let feats = feats_of(&state, &a());
    assert!(!feats.contains(&ti4_model::state::Feat::WonInAnAnomaly) && feats.contains(&ti4_model::state::Feat::DestroyedACapitalShip), "{feats:?}");
    let (state, noted, r) = feats_run(&system, Some(a()), 0, standard, b_loses_everything);
    assert!(!noted && feats_of(&state, &a()).is_empty());
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
}

const OVER: &str = "aftermath::fight_over";

fn over_run(seed: u64, winner: Option<PlayerId>, rounds: u32, setup: impl Fn(&mut GameState, &SystemId)) -> (GameState, (bool, bool), Vec<String>, crate::dual::DualReport) {
    over_run_against(seed, winner, rounds, (b(), "cruiser"), setup)
}

/// [`over_run`] with the losing side's owner and ship type named.
fn over_run_against(seed: u64, winner: Option<PlayerId>, rounds: u32, loser: (PlayerId, &str), setup: impl Fn(&mut GameState, &SystemId)) -> (GameState, (bool, bool), Vec<String>, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    state.rng_seed = seed;
    let (system, _) = crate::fixtures::a_placed_planet();
    state.active = Some(a());
    state.active_system = Some(system.clone());
    crate::fixtures::put(&mut state, &system, "cruiser", &a(), 2);
    crate::fixtures::put(&mut state, &system, loser.1, &loser.0, 2);
    let content = ContentStore::embedded();
    let before = crate::combat::before_combat(&state, content, ti4_model::content_types::DEFAULT, &system);
    state.system_mut(&system).units.retain(|u| u.owner != loser.0);
    state.active_space_combat = Some((system.clone(), a(), loser.0.clone()));
    setup(&mut state, &system);
    let outcome = crate::combat::CombatOutcome { winner, rounds };
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let mut log = Vec::new();
    let result = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        AftermathWindow::fight_over(&mut state, &mut ctx, &a(), &system, &before, &outcome, &mut log).unwrap()
    });
    let r = report();
    let _ = take_report();
    (state, result, log, r)
}

#[test]
fn the_end_of_a_won_fight_runs_in_the_core_with_the_win_the_scratch_and_the_enforcement() {
    let (state, (holds, contacts), log, r) = over_run(1, Some(a()), 2, |s, system| {
        // Fighters with no carrier are over capacity: the owner is asked which go.
        crate::fixtures::put(s, system, "fighter", &a(), 7);
    });
    assert_eq!((holds, contacts), (true, false), "a still has ships there");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(OVER).map_or(0, |w| w.compared) >= 1, "the core window ran: {:?}", r.windows);
    assert!(state.active_space_combat.is_none() && state.last_combat_sides.as_ref().is_some_and(|(_, sides)| sides == &vec![b()]));
    assert_eq!(log, ["SPACE_COMBAT_RESOLVED"]);
    assert!(state.system_state(&crate::fixtures::a_placed_planet().0).units.len() < 9, "the stranded fighters went");
}

/// Package W11: the end of a fight won against the neutral force (its last ship fell) runs in the core, with the neutral owner in the
/// table although the board holds none of its units any more. Fails on the pre-W11 gate ("a neutral side").
#[test]
fn the_end_of_a_fight_won_against_the_neutral_force_runs_in_the_core() {
    let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
    let (state, (holds, contacts), log, r) = over_run_against(1, Some(a()), 2, (neutral.clone(), "neutral_cruiser"), |_, _| {});
    assert_eq!((holds, contacts), (true, false), "a still has ships there");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(OVER).map_or(0, |w| w.compared) >= 1, "the core window ran: {:?}", r.windows);
    assert!(r.windows.values().all(|w| w.bailed == 0), "no bail: {:?}", r.windows);
    assert!(state.last_combat_sides.as_ref().is_some_and(|(_, sides)| sides == &vec![neutral]), "{:?}", state.last_combat_sides);
    assert_eq!(log, ["SPACE_COMBAT_RESOLVED"]);
}

/// Package W11: the guns of the aftermath prelude fire with the neutral force (Pirate Fleet) in the system: its ships are other units on
/// the board, not gunners. Fails on the pre-W11 gate ("neutral units in the system").
#[test]
fn the_space_cannon_offense_runs_in_the_core_with_the_neutral_force_in_the_system() {
    let (ran, r) = lockstep(0..10, &[("a", "sol"), ("b", "hacan")], |s, system, planet| {
        pds_at_b(s, system, planet);
        let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
        crate::fixtures::put(s, system, "neutral_cruiser", &neutral, 1);
        crate::fixtures::put(s, system, "neutral_fighter", &neutral, 2);
        crate::fixtures::put_on_planet(s, system, planet, "neutral_infantry", &neutral, 1);
    });
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 8, "the core window ran: {:?}", r.windows);
    assert!(r.windows.values().all(|w| w.bailed == 0), "no bail: {:?}", r.windows);
    assert!(ran.iter().all(|x| x.dice > 0), "the guns rolled");
}

#[test]
fn a_fight_of_no_rounds_and_a_draw_open_no_win_window_and_a_lost_fight_does_not_hold_the_space() {
    let (state, (holds, _), log, r) = over_run(2, None, 0, |_, _| {});
    assert!(holds);
    assert!(log.is_empty() && state.last_combat_sides.is_none());
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let (_, (holds, _), log, r) = over_run(3, None, 2, |s, system| s.system_mut(system).units.retain(|u| u.owner != a()));
    assert!(!holds, "a has nothing left there");
    assert_eq!(log, ["SPACE_COMBAT_RESOLVED"]);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(OVER).map_or(0, |w| w.compared) >= 1, "{:?}", r.windows);
}

#[test]
fn shard_of_the_throne_and_a_titans_seat_keep_the_end_of_the_fight_on_the_string_and_say_why() {
    let (_, _, _, r) = over_run(4, Some(a()), 2, |s, _| {
        s.laws.insert("shard_of_the_throne".to_owned(), "b".to_owned());
    });
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.iter().any(|(k, w)| k.starts_with(OVER) && k.contains("[bail: Shard of the Throne") && w.bailed > 0), "{:?}", r.windows);
}

/// The head with the second seat active: the attacker is the active seat, not the first of the sides.
#[test]
fn the_head_makes_the_active_seat_the_attacker_even_when_it_sits_second() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut sides = Vec::new();
    for seed in 0..6_u64 {
        let (mut state, system) = position(seed, &[("a", "sol"), ("b", "hacan")], |s, system, planet| {
            crate::fixtures::put_on_planet(s, system, planet, "pds", &a(), 1);
            crate::fixtures::put(s, system, "cruiser", &a(), 3);
            crate::fixtures::put(s, system, "cruiser", &b(), 3);
        });
        state.active = Some(b());
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
            };
            let notes = crate::combat::note_holdings(&state);
            AftermathWindow::head(&mut state, &mut ctx, &b(), &system, None, &notes, false).unwrap();
        });
        sides.push(state.active_space_combat.clone().map(|(_, attacker, defender)| (attacker, defender)));
    }
    let r = report();
    let _ = take_report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(HEAD).map_or(0, |w| w.compared) >= 4, "{:?}", r.windows);
    assert!(sides.iter().flatten().all(|(attacker, defender)| *attacker == b() && *defender == a()), "{sides:?}");
    assert!(sides.iter().any(Option::is_some), "a fight opened");
}

#[test]
fn a_fight_of_no_rounds_opens_no_win_window_even_with_a_winner_named() {
    let (state, _, log, r) = over_run(5, Some(a()), 0, |_, _| {});
    assert!(log.is_empty() && state.last_combat_sides.is_none(), "{log:?}");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(OVER).map_or(0, |w| w.compared) >= 1, "{:?}", r.windows);
}

#[test]
fn a_fight_of_one_round_is_a_fight_and_an_activator_without_ships_attacks_with_the_first_side() {
    let (state, _, log, r) = over_run(6, Some(a()), 1, |_, _| {});
    assert_eq!(log, ["SPACE_COMBAT_RESOLVED"]);
    assert!(state.last_combat_sides.is_some());
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    // c activated the system but has no ship in it: a, the first of the two sides, is the attacker.
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let (mut state, system) = position(1, &[("a", "sol"), ("b", "hacan"), ("c", "letnev")], |s, system, _| {
        crate::fixtures::put(s, system, "cruiser", &a(), 2);
        crate::fixtures::put(s, system, "cruiser", &b(), 2);
    });
    let c = PlayerId::new("c");
    state.active = Some(c.clone());
    with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let (mut dice, mut rng) = (Dice::new(), GameRng::new(1));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        let notes = crate::combat::note_holdings(&state);
        AftermathWindow::head(&mut state, &mut ctx, &c, &system, None, &notes, false).unwrap();
    });
    let r = report();
    let _ = take_report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get(HEAD).map_or(0, |w| w.compared) >= 1, "{:?}", r.windows);
    assert_eq!(state.active_space_combat.map(|(_, attacker, defender)| (attacker, defender)), Some((a(), b())));
}

/// A Naaz planetary Maximum is a combat unit only beside a ship and is immune to the hits of a unit ability: the guns' hits go to
/// the other ships (the `ability` flag of the absorption).
#[test]
fn a_planetary_maximum_is_never_assigned_the_hits_of_the_guns_in_lockstep() {
    let naaz = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 3);
        crate::fixtures::put_on_planet(s, system, planet, "naaz_voltron", &a(), 1);
        crate::fixtures::put(s, system, "cruiser", &a(), 1);
    };
    let (ran, r) = lockstep(0..30, &[("a", "naaz"), ("b", "hacan")], naaz);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 20, "the core window ran: {:?}", r.windows);
    assert!(
        ran.iter().all(|x| {
            let (system, planet) = crate::fixtures::a_placed_planet();
            x.state.system_state(&system).on_planet(&planet).iter().any(|u| u.type_id.as_str() == "naaz_voltron")
        }),
        "the Maximum was never lost to the guns"
    );
}

/// A Yssaril seat with Ssruu readied can copy the agent of another seat, but only the Sardakk agent makes the end of a tactical
/// action a window for T'ro: the core is told whether Ssruu lends THAT agent, not whether it lends any.
#[test]
fn ssruu_borrowing_another_agent_does_not_open_the_tro_window() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut state = crate::fixtures::seated_game(&[("a", "yssaril"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    let content = ContentStore::embedded();
    assert!(
        !crate::factions::hooks_cards::borrowable_agents(&state, content, &a()).is_empty(),
        "Ssruu is readied and b's agent can be copied"
    );
    let mut game = super::Game::with_table(state, content, Table::with_default(Box::new(crate::choice::FirstOption)));
    game.state.active = Some(a());
    with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || game.check_close_gates());
    let r = report();
    let _ = take_report();
    assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.compared.iter().sum::<u64>()) >= 2, "the gates and the continuation were compared: {:?} {:?}", r.areas, r.windows);
    assert!(!r.windows.keys().any(|k| k.starts_with("turn::close_gates [bail")), "{:?}", r.windows);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
}
