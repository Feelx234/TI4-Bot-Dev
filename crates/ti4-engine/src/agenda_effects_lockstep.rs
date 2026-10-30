//! The table effects of `rules::agenda::table_effects` run as core windows beside the string effect
//! (`agenda=dual`): the same state, the same ledger, the same staged events and ground rows. Every scenario is
//! counted as compared, never bailed, and the bails of the table are documented here, one test each.

use ti4_model::content_types::POK;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::{GameState, Phase};
use ti4_model::units::Unit;

use super::{resolve, Ballot, Effect, AGAINST, FOR};
use crate::dual::{local_windows, take_report, with_mask, AreaMask, Backend};

fn pid(name: &str) -> PlayerId {
    PlayerId::new(name)
}

fn system_of(planet: &str) -> SystemId {
    let planet = ti4_content::galaxy::planet(ti4_content::ContentStore::embedded(), planet, POK).expect("a printed planet");
    SystemId::new(planet.system_id().expect("a printed system"))
}

/// Units (by type id) on a planet, which `owner` takes over.
fn garrison(state: &mut GameState, planet: &str, owner: &str, units: &[&str]) {
    let (system, planet) = (system_of(planet), PlanetId::new(planet));
    state.system_mut(&system).planet_control.insert(planet.clone(), pid(owner));
    for unit in units {
        state.system_mut(&system).planet_units.entry(planet.clone()).or_default().push(Unit::new(UnitTypeId::new(*unit), pid(owner)));
    }
}

fn ships(state: &mut GameState, system: &str, owner: &str, units: &[&str]) {
    for unit in units {
        state.system_mut(&SystemId::new(system)).units.push(Unit::new(UnitTypeId::new(*unit), pid(owner)));
    }
}

fn ballot(votes: &[(&str, &str)]) -> Ballot {
    Ballot { votes: votes.iter().map(|(p, v)| (pid(p), (*v).to_owned())).collect(), counts: Default::default() }
}

fn hand(state: &mut GameState, who: &str, cards: &[&str]) {
    state.player_mut(&pid(who)).unwrap().action_cards = cards.iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
}

/// A three-seat game in the agenda phase: sol (a, speaker), letnev (b), hacan (c).
fn table() -> GameState {
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev"), ("c", "hacan")], POK);
    state.phase = Phase::Agenda;
    state.speaker = pid("a");
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    for who in ["a", "b", "c"] {
        hand(&mut state, who, &["sabo1", "sabo2", "sabo3"]);
    }
    state
}

fn mask() -> AreaMask {
    AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap()
}

fn run(state: &mut GameState, agenda: &str, outcome: &str, ballot: &Ballot) -> Effect {
    resolve(state, ti4_content::ContentStore::embedded(), agenda, outcome, ballot)
}

/// Run `body` on the dual and return how many windows the core compared and bailed, no difference allowed.
fn counted(body: impl FnOnce()) -> (u64, u64) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), body);
    let report = take_report();
    assert_eq!(report.total_differences(), 0, "{report:?}");
    for (label, w) in &report.windows {
        if w.bailed > 0 {
            eprintln!("bail: {label} x{}", w.bailed);
        }
    }
    local_windows("agenda_effects::resolve_with")
}

#[test]
fn the_token_and_goods_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        run(&mut state, "conscription", FOR, &Ballot::default());
        run(&mut state, "conscription", AGAINST, &Ballot::default());
        // Standardization tops up and never takes away.
        {
            let seat = state.player_mut(&pid("a")).unwrap();
            seat.tactic_tokens = 1;
            seat.fleet_tokens = 5;
            seat.strategic_tokens = 2;
        }
        run(&mut state, "standardization", "a", &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().tactic_tokens, 3);
        assert_eq!(state.player(&pid("a")).unwrap().fleet_tokens, 5);
        // Clandestine Operations: two from the fullest pool (the last of equals), or one fleet token.
        state.player_mut(&pid("b")).unwrap().tactic_tokens = 4;
        state.player_mut(&pid("b")).unwrap().fleet_tokens = 4;
        state.player_mut(&pid("b")).unwrap().strategic_tokens = 0;
        // (1, 1, 1): the last of the fullest pools goes first, so the leftover is the tactic pool.
        for (n, pool) in [(1, "tactic"), (1, "fleet"), (1, "strategic")] {
            let seat = state.player_mut(&pid("c")).unwrap();
            match pool {
                "tactic" => seat.tactic_tokens = n,
                "fleet" => seat.fleet_tokens = n,
                _ => seat.strategic_tokens = n,
            }
        }
        run(&mut state, "cladenstine", FOR, &Ballot::default());
        assert_eq!(state.player(&pid("c")).unwrap().tactic_tokens, 1);
        run(&mut state, "cladenstine", AGAINST, &Ballot::default());
        state.player_mut(&pid("c")).unwrap().fleet_tokens = 0;
        run(&mut state, "cladenstine", AGAINST, &Ballot::default());
        // Fleet Regulations: a fleet token each, but not past the reinforcements (a Mahact pool counts).
        {
            let seat = state.player_mut(&pid("a")).unwrap();
            (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (0, 0, 0);
        }
        // Exactly one token is left in the reinforcements, and one sits in a Mahact fleet pool.
        let free = state.tokens_in_reinforcements(&pid("a"));
        state.player_mut(&pid("a")).unwrap().tactic_tokens = free - 1;
        assert_eq!(state.tokens_in_reinforcements(&pid("a")), 1);
        state.faction_marks.insert("mahact:fleet:c".to_owned(), "a".to_owned());
        run(&mut state, "regulations", FOR, &Ballot::default());
        run(&mut state, "regulations", AGAINST, &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().fleet_tokens, 0, "the Mahact pool holds the last one");
        // Economic Equality and Articles of War.
        state.player_mut(&pid("a")).unwrap().trade_goods = 4;
        run(&mut state, "economic_equality", AGAINST, &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().trade_goods, 0);
        run(&mut state, "economic_equality", FOR, &Ballot::default());
        assert_eq!(state.player(&pid("c")).unwrap().trade_goods, 5);
        let votes = ballot(&[("a", FOR), ("c", FOR), ("b", AGAINST)]);
        run(&mut state, "articles_war", AGAINST, &votes);
        assert_eq!(state.player(&pid("a")).unwrap().trade_goods, 8);
        assert_eq!(state.player(&pid("b")).unwrap().trade_goods, 5);
        run(&mut state, "articles_war", FOR, &votes);
    });
    assert!(compared >= 12 && bailed == 0, "{compared} compared, {bailed} bailed");
}

#[test]
fn the_hand_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        // Public Execution: the speaker is elected and the token moves clockwise; another seat only loses its hand.
        let effect = run(&mut state, "execution", "a", &Ballot::default());
        assert!(matches!(effect, Effect::Deferred { .. }));
        assert_eq!(state.speaker, pid("b"));
        assert!(state.player(&pid("a")).unwrap().action_cards.is_empty());
        run(&mut state, "execution", "c", &Ballot::default());
        assert_eq!(state.speaker, pid("b"));
        // Conventions of War: the Against voters.
        let votes = ballot(&[("a", AGAINST), ("b", FOR), ("c", AGAINST)]);
        hand(&mut state, "a", &["sabo1"]);
        run(&mut state, "conventions", FOR, &votes);
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 1);
        run(&mut state, "conventions", AGAINST, &votes);
        assert!(state.player(&pid("a")).unwrap().action_cards.is_empty());
        assert_eq!(state.player(&pid("b")).unwrap().action_cards.len(), 3);
        // Unconventional Measures, Against: the For voters lose their hands.
        run(&mut state, "unconventional", AGAINST, &votes);
        assert!(state.player(&pid("b")).unwrap().action_cards.is_empty());
        // Publicize Weapon Schematics: only the War Sun owners.
        hand(&mut state, "a", &["sabo1"]);
        hand(&mut state, "b", &["sabo1"]);
        hand(&mut state, "c", &["sabo1"]);
        state.player_mut(&pid("b")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("pws2"));
        state.player_mut(&pid("c")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ws"));
        run(&mut state, "schematics", FOR, &Ballot::default());
        assert_eq!(state.player(&pid("b")).unwrap().action_cards.len(), 1);
        run(&mut state, "schematics", AGAINST, &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 1);
        assert!(state.player(&pid("b")).unwrap().action_cards.is_empty());
        assert!(state.player(&pid("c")).unwrap().action_cards.is_empty());
    });
    assert!(compared >= 7 && bailed == 0, "{compared} compared, {bailed} bailed");
}

#[test]
fn the_command_token_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        // One seat keeps a home system of its own; another has none recorded and takes its faction's.
        state.player_mut(&pid("b")).unwrap().home_system = Some(SystemId::new("27"));
        state.player_mut(&pid("c")).unwrap().home_system = None;
        run(&mut state, "shared_research", FOR, &Ballot::default());
        run(&mut state, "shared_research", AGAINST, &Ballot::default());
        assert!(state.system_state(&SystemId::new("27")).command_tokens.contains(&pid("b")));
        // A second run places nothing new.
        run(&mut state, "shared_research", AGAINST, &Ballot::default());
        // Wormhole Reconstruction: only wormhole systems holding a ship; tile 25 is an alpha wormhole, 26 beta.
        ships(&mut state, "25", "a", &["carrier"]);
        ships(&mut state, "26", "b", &["cruiser"]);
        ships(&mut state, "26", "c", &["destroyer"]);
        ships(&mut state, "27", "a", &["carrier"]);
        run(&mut state, "wormhole_recon", FOR, &Ballot::default());
        run(&mut state, "wormhole_recon", AGAINST, &Ballot::default());
        assert!(state.system_state(&SystemId::new("26")).command_tokens.contains(&pid("b")));
        assert!(!state.system_state(&SystemId::new("27")).command_tokens.contains(&pid("a")));
    });
    assert!(compared >= 5 && bailed == 0, "{compared} compared, {bailed} bailed");
}

#[test]
fn the_unit_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        // Arms Reduction: down to two dreadnoughts and four cruisers each, board order decides which go.
        ships(&mut state, "25", "a", &["dreadnought", "dreadnought", "cruiser", "cruiser", "carrier"]);
        ships(&mut state, "26", "a", &["dreadnought", "cruiser", "cruiser", "cruiser"]);
        ships(&mut state, "27", "a", &["dreadnought", "cruiser"]);
        ships(&mut state, "27", "b", &["dreadnought", "cruiser"]);
        run(&mut state, "arms_reduction", AGAINST, &Ballot::default());
        run(&mut state, "arms_reduction", FOR, &Ballot::default());
        let left = |s: &GameState, base: &str| s.board.values().flat_map(|b| b.units.iter()).filter(|u| u.owner == pid("a") && u.type_id.as_str() == base).count();
        assert_eq!((left(&state, "dreadnought"), left(&state, "cruiser")), (2, 4));
        // Rearmament, Against: every mech on a planet is an infantry again.
        garrison(&mut state, "jord", "a", &["infantry", "mech", "mech"]);
        garrison(&mut state, "arcprime", "b", &["mech", "pds"]);
        state.system_mut(&system_of("jord")).planet_units.get_mut(&PlanetId::new("jord")).unwrap()[1].sustained_damage = true;
        run(&mut state, "rearmament", AGAINST, &Ballot::default());
        assert!(state.system_state(&system_of("jord")).planet_units[&PlanetId::new("jord")].iter().all(|u| !u.type_id.as_str().ends_with("mech")));
    });
    assert!(compared >= 3 && bailed == 0, "{compared} compared, {bailed} bailed");
}

#[test]
fn the_planet_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        garrison(&mut state, "jord", "a", &["infantry", "mech", "infantry", "pds", "spacedock"]);
        garrison(&mut state, "arcprime", "b", &["infantry", "infantry", "infantry", "mech"]);
        garrison(&mut state, "arretze", "c", &["infantry", "infantry", "pds"]);
        // Core Mining: one infantry only.
        let before = state.system_state(&system_of("jord")).planet_units[&PlanetId::new("jord")].len();
        run(&mut state, "core_mining", "jord", &Ballot::default());
        assert_eq!(state.system_state(&system_of("jord")).planet_units[&PlanetId::new("jord")].len(), before - 1);
        // Compensated Disarmament: the garrison is bought out for goods.
        run(&mut state, "disarmament", "arcprime", &Ballot::default());
        assert!(state.player(&pid("b")).unwrap().trade_goods >= 4);
        // One ground force bought out pays one good.
        garrison(&mut state, "abaddon", "a", &["infantry"]);
        let goods = state.player(&pid("a")).unwrap().trade_goods;
        run(&mut state, "disarmament", "abaddon", &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().trade_goods, goods + 1);
        // Swords to Plowshares: half of every seat's infantry, rounded up.
        run(&mut state, "plowshares", FOR, &Ballot::default());
        // Demilitarized Zone: everything dies.
        run(&mut state, "demilitarized_zone", "jord", &Ballot::default());
        assert!(state.system_state(&system_of("jord")).planet_units[&PlanetId::new("jord")].is_empty());
        // A planet nobody has touched: nothing to clear.
        run(&mut state, "demilitarized_zone", "ashtroth", &Ballot::default());
        run(&mut state, "disarmament", "ashtroth", &Ballot::default());
    });
    assert!(compared >= 6 && bailed == 0, "{compared} compared, {bailed} bailed");
}

/// Unconventional Measures For (two action cards for each For voter) and Minister of Antiquities (the top relic).
#[test]
fn the_draw_and_relic_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        for who in ["a", "b", "c"] {
            hand(&mut state, who, &[]);
        }
        state.action_card_deck = ["sabo1", "sabo2", "sabo3"].iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
        let votes = ballot(&[("a", FOR), ("b", AGAINST), ("c", FOR)]);
        run(&mut state, "unconventional", FOR, &votes);
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 2);
        assert_eq!(state.player(&pid("c")).unwrap().action_cards.len(), 1, "the deck ran out");
        assert!(state.player(&pid("b")).unwrap().action_cards.is_empty());
        // Minister of Antiquities: the elected seat takes the top relic; the Shard scores at once.
        state.relic_deck = ["dynamiscore", "shard"].iter().map(|r| ti4_model::id::RelicId::new(*r)).collect();
        run(&mut state, "minister_antiquities", "b", &Ballot::default());
        assert_eq!(state.player(&pid("b")).unwrap().relics.len(), 1);
        run(&mut state, "minister_antiquities", "b", &Ballot::default());
        assert_eq!(state.player(&pid("b")).unwrap().victory_points, 1);
        // An empty deck yields nothing (73.2a).
        run(&mut state, "minister_antiquities", "c", &Ballot::default());
        assert!(state.player(&pid("c")).unwrap().relics.is_empty());
    });
    assert!(compared >= 4 && bailed == 0, "{compared} compared, {bailed} bailed");
}

/// A draw that a faction hook changes (Yssaril's Scheming) asks questions of its own: the Unconventional Measures window is
/// dropped (B9c-2). A draw that leaves the hand over its limit is the core's since W6 (the child draw window asks the discard).
#[test]
fn a_hooked_unconventional_draw_has_no_window() {
    let mut state = table();
    hand(&mut state, "a", &[]);
    assert!(core_half(&mut state, "unconventional", FOR, &ballot(&[("a", FOR)])).is_some());
    let mut state = crate::fixtures::seated_game(&[("a", "yssaril"), ("b", "letnev"), ("c", "hacan")], POK);
    state.phase = Phase::Agenda;
    assert!(core_half(&mut state, "unconventional", FOR, &ballot(&[("a", FOR)])).is_none(), "Scheming");
    // Another seat's Yssaril changes nothing for the voter (W6: it was a bail for every voter).
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "yssaril"), ("c", "hacan")], POK);
    state.phase = Phase::Agenda;
    assert!(core_half(&mut state, "unconventional", FOR, &ballot(&[("a", FOR)])).is_some(), "a Yssaril beside the voter");
    // Against stays a hand effect and is not affected.
    let mut state = crate::fixtures::seated_game(&[("a", "yssaril"), ("b", "letnev"), ("c", "hacan")], POK);
    state.phase = Phase::Agenda;
    assert!(core_half(&mut state, "unconventional", AGAINST, &ballot(&[("a", FOR)])).is_some());
}

/// W6 regression: a For voter whose hand ends over the limit is asked which card to discard (the child draw window), voter
/// after voter; the string asks the same question (it was `unsupported`, a bail of the window).
#[test]
fn an_unconventional_draw_over_the_hand_limit_is_asked_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut state = table();
        hand(&mut state, "a", &["sabo1", "sabo1", "sabo1", "sabo1", "sabo1", "sabo1"]);
        hand(&mut state, "b", &["sabo2", "sabo2", "sabo2", "sabo2", "sabo2", "sabo2"]);
        let votes = ballot(&[("a", FOR), ("b", FOR)]);
        run(&mut state, "unconventional", FOR, &votes);
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 7);
        assert_eq!(state.player(&pid("b")).unwrap().action_cards.len(), 7);
    });
    assert_eq!((compared, bailed), (1, 0));
}

/// The core half of an effect, probed on its own: `None` is a bail (the string effect runs alone).
fn core_half(state: &mut GameState, agenda: &str, outcome: &str, votes: &Ballot) -> Option<bool> {
    let (for_voters, against_voters) = (votes.voted_for(FOR), votes.voted_for(AGAINST));
    let args = crate::dual_window::EffectArgs { for_voters: &for_voters, against_voters: &against_voters, sources: POK, galaxy: None, rolls: None };
    crate::dual_window::agenda_effect(state, agenda, outcome, &args, &crate::dual_window::Recording::default()).map(|w| w.run.value.0)
}

/// Resolve with the game's roller seeded `seed` (the state's seed follows, so the streams are the ones the core copies).
fn run_seeded(state: &mut GameState, agenda: &str, outcome: &str, seed: u64) -> Effect {
    state.rng_seed = seed;
    let content = ti4_content::ContentStore::embedded();
    let mut dice = crate::dice::Dice::new();
    let mut rng = crate::rng::GameRng::new(seed);
    let mut table = crate::choice::Table::new();
    let mut ctx = crate::choice::Resolving { content, sources: POK, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
    crate::agenda_effects::resolve_with(state, &mut ctx, None, agenda, outcome, &Ballot::default())
}

/// Executive Sanctions (Against) discards a random card of each hand and Ixthian Artifact (For) either pays out
/// (a 6, deferred: it needs a payer) or destroys every ship that cannot sustain damage, from the game's seeded dice:
/// the core window draws from its copy of the streams and the faces and positions are compared.
#[test]
fn the_dice_effects_run_as_core_windows_in_lockstep() {
    let (compared, bailed) = counted(|| {
        let mut deferred = 0;
        let mut cratered = 0;
        for seed in 0..24 {
            let mut state = table();
            hand(&mut state, "a", &["sabo1", "sabo2", "sabo3"]);
            hand(&mut state, "b", &[]);
            hand(&mut state, "c", &["veto", "hack"]);
            run_seeded(&mut state, "sanctions", AGAINST, seed);
            assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 2, "seed {seed}");
            assert!(state.player(&pid("b")).unwrap().action_cards.is_empty());
            assert_eq!(state.player(&pid("c")).unwrap().action_cards.len(), 1);
            // For: nothing happens, and nothing is rolled.
            run_seeded(&mut state, "sanctions", FOR, seed);
            assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 2);

            let mut state = table();
            ships(&mut state, "27", "a", &["carrier", "dreadnought", "fighter", "destroyer"]);
            ships(&mut state, "28", "b", &["cruiser", "warsun"]);
            let effect = run_seeded(&mut state, "artifact", FOR, seed);
            let left = |system: &str| state.system_state(&SystemId::new(system)).units.len();
            match effect {
                Effect::Deferred { .. } => {
                    deferred += 1;
                    assert_eq!((left("27"), left("28")), (4, 2));
                }
                _ => {
                    cratered += 1;
                    assert_eq!((left("27"), left("28")), (1, 1), "only the dreadnought and the war sun survive");
                }
            }
            run_seeded(&mut state, "artifact", AGAINST, seed);
        }
        assert!(deferred > 0 && cratered > 0, "both branches ran: {deferred} deferred, {cratered} cratered");
    });
    assert!(compared >= 24 * 4 && bailed == 0, "{compared} compared, {bailed} bailed");
}

/// The streams are copied at the position the string's stood at, not at the start.
#[test]
fn the_dice_effects_continue_the_streams_where_they_stood() {
    let (compared, bailed) = counted(|| {
        for seed in 0..8 {
            let mut state = table();
            hand(&mut state, "a", &["sabo1", "sabo2", "sabo3"]);
            state.rng_seed = seed;
            let content = ti4_content::ContentStore::embedded();
            let (mut dice, mut rng, mut t) = (crate::dice::Dice::new(), crate::rng::GameRng::new(seed), crate::choice::Table::new());
            dice.roll(&mut rng, 7, "warm up", None);
            let mut ctx = crate::choice::Resolving { content, sources: POK, dice: &mut dice, rng: &mut rng, table: &mut t, timing: None };
            crate::agenda_effects::resolve_with(&mut state, &mut ctx, None, "sanctions", AGAINST, &Ballot::default());
            assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 2);
        }
    });
    assert_eq!((compared, bailed), (8, 0));
}

/// Dice the core cannot reproduce (test faces, a restore, a seed other than the state's) run the string body bare
/// (B9-bail-01).
#[test]
fn irreproducible_dice_have_no_window() {
    let mut state = table();
    // The probe passes no stream positions: the same as test faces or a pending restore.
    assert!(core_half(&mut state, "sanctions", AGAINST, &Ballot::default()).is_none());
    assert!(core_half(&mut state, "artifact", FOR, &Ballot::default()).is_none());
    // A seed that is not the state's: the streams are not the ones the core copies.
    let (compared, bailed) = counted(|| {
        let mut state = table();
        hand(&mut state, "a", &["sabo1", "sabo2"]);
        state.rng_seed = 5;
        let content = ti4_content::ContentStore::embedded();
        let (mut dice, mut rng, mut t) = (crate::dice::Dice::new(), crate::rng::GameRng::new(9), crate::choice::Table::new());
        let mut ctx = crate::choice::Resolving { content, sources: POK, dice: &mut dice, rng: &mut rng, table: &mut t, timing: None };
        crate::agenda_effects::resolve_with(&mut state, &mut ctx, None, "sanctions", AGAINST, &Ballot::default());
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 1);
        let key = "agenda_effects::resolve_with [bail: irreproducible dice (test faces or a stream restore)]";
        assert_eq!(crate::dual::report().windows.get(key).map(|w| w.bailed), Some(1));
    });
    assert_eq!((compared, bailed), (0, 1));
    // The For half of Sanctions and the Against half of the Artifact roll nothing and are plain windows.
    assert!(core_half(&mut state, "sanctions", FOR, &Ballot::default()).is_some());
    assert!(core_half(&mut state, "artifact", AGAINST, &Ballot::default()).is_some());
}

/// Rearmament For and Swords to Plowshares Against place units through the supply, the unit-form flip and the
/// faction placement hooks, and Unconventional Measures For draws announced action cards (B9-bail-02).
#[test]
fn the_placing_and_drawing_halves_have_no_window() {
    let mut state = table();
    assert!(core_half(&mut state, "rearmament", FOR, &Ballot::default()).is_none());
    assert!(core_half(&mut state, "plowshares", AGAINST, &Ballot::default()).is_none());
    // Colonial Redistribution places an infantry through the supply and the faction hooks.
    assert!(core_half(&mut state, "redistribution", "jord", &Ballot::default()).is_none());
    // The halves that are ported answer.
    assert!(core_half(&mut state, "rearmament", AGAINST, &Ballot::default()).is_some());
    assert!(core_half(&mut state, "plowshares", FOR, &Ballot::default()).is_some());
}

/// The relic deck (Minister of Antiquities), the technology question (Research Grant Reallocation) and Galactic
/// Crisis Pact (a question per seat, then a strategy card's secondary) stay string (B9-bail-03).
#[test]
fn the_relic_technology_and_crisis_effects_have_no_window() {
    let mut state = table();
    assert!(core_half(&mut state, "grant_reallocation", "a", &Ballot::default()).is_none());
    assert!(core_half(&mut state, "crisis", "leadership", &Ballot::default()).is_none());
}

/// An agenda effect in the action phase can be taken by the Ral Nel breakthrough on a discard: the core knows no
/// such rule and declines the phase (B9-bail-04).
#[test]
fn the_action_phase_has_no_window() {
    let mut state = table();
    assert!(core_half(&mut state, "execution", "b", &Ballot::default()).is_some());
    state.phase = Phase::Action;
    assert!(core_half(&mut state, "execution", "b", &Ballot::default()).is_none());
}

/// A galvanized ground force that falls stages a row with one more field than the typed mark holds: the window
/// is dropped after it ran (B9-bail-05).
#[test]
fn a_galvanized_ground_force_drops_the_window() {
    let mut state = table();
    garrison(&mut state, "arcprime", "b", &["infantry"]);
    let system = system_of("arcprime");
    let planet = PlanetId::new("arcprime");
    state.system_mut(&system).planet_units.get_mut(&planet).unwrap().last_mut().unwrap().galvanized = true;
    assert!(core_half(&mut state, "demilitarized_zone", "arcprime", &Ballot::default()).is_none());
    // The same planet without it is followed.
    let mut state = table();
    garrison(&mut state, "arcprime", "b", &["infantry"]);
    assert!(core_half(&mut state, "demilitarized_zone", "arcprime", &Ballot::default()).is_some());
}
