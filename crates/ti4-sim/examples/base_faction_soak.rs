//! Deterministic seated soak for the twelve base-faction packages.
//!
//! Run all 200 seeds per faction with:
//! cargo run --release -p ti4-sim --example base_faction_soak
//!
//! Resume a slice with: ... -- <faction> <first_seed> <count>
//! The target replaces one of six original factions, rotating its seat by seed.
//! No production faction-scope list is widened.

use std::collections::BTreeMap;
use std::io::{self, Write};

use ti4_content::ContentStore;
use ti4_engine::choice::DecisionRecord;
use ti4_engine::game::Game;
use ti4_engine::setup::start_game_seeded;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{FactionId, PlayerId};

const FACTIONS: [&str; 12] = [
    "arborec", "argent", "ghost", "mentak", "muaat", "naalu", "naaz", "saar", "sardakk", "winnu",
    "yin", "yssaril",
];
const SEATS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];
const MAX_ROUNDS: u32 = 50;
const MAX_STEPS: usize = 25_000;

#[derive(PartialEq)]
struct Replay {
    state: serde_json::Value,
    events: Vec<String>,
    decisions: Vec<DecisionRecord>,
}

fn play(content: &ContentStore, faction: &str, seed: u64) -> Result<Replay, String> {
    let players: Vec<PlayerId> = SEATS.iter().map(|name| PlayerId::new(*name)).collect();
    let mut assignments = ti4_engine::seating::seat_in_scope(&players);
    let target = &players[usize::try_from(seed % 6).map_err(|error| error.to_string())?];
    assignments.insert(target.clone(), FactionId::new(faction));

    let mut state = start_game_seeded(content, &players, DEFAULT, None, seed)
        .map_err(|error| format!("setup: {error}"))?;
    for (player, assigned) in &assignments {
        state
            .player_mut(player)
            .ok_or_else(|| format!("missing seat {player}"))?
            .faction = assigned.clone();
    }

    let filler: Vec<String> = ti4_engine::seating::map_filler(content, 30, DEFAULT, seed)
        .into_iter()
        .map(|system| system.to_string())
        .collect();
    let borrowed: Vec<&str> = filler.iter().map(String::as_str).collect();
    let galaxy = ti4_engine::seating::build_board(content, &assignments, &borrowed, DEFAULT)
        .map_err(|error| format!("board: {error}"))?;
    for (player, assigned) in &assignments {
        ti4_engine::seating::deploy(&mut state, content, player, assigned, DEFAULT)
            .map_err(|error| format!("deploy {player} as {assigned}: {error}"))?;
    }

    let mut game = Game::with_seeded_random(state, content, seed)
        .with_sources(DEFAULT)
        .with_galaxy(galaxy);
    if let Err(error) = game.run(MAX_ROUNDS, MAX_STEPS) {
        let last = game.table.log.records.last().map_or_else(
            || "<none>".to_owned(),
            |record| format!("{}: {} -> {}", record.player, record.prompt, record.chosen),
        );
        let pending = game.legal_options().map_or_else(
            || "<none>".to_owned(),
            |choice| {
                format!(
                    "{}: {} [{} options]",
                    choice.player,
                    choice.prompt,
                    choice.options.len()
                )
            },
        );
        return Err(format!(
            "round={} phase={:?} error={error}; last_decision={last}; pending={pending}",
            game.state.round, game.state.phase
        ));
    }
    if !game.state.finished {
        return Err(format!(
            "horizon reached without a finished game: round={} decisions={}",
            game.state.round,
            game.table.log.records.len()
        ));
    }
    Ok(Replay {
        state: serde_json::to_value(&game.state).map_err(|error| error.to_string())?,
        events: game.events,
        decisions: game.table.log.records,
    })
}

fn run_faction(content: &ContentStore, faction: &str, start: u64, count: u64) -> usize {
    let mut failures = 0;
    for seed in start..start.saturating_add(count) {
        match play(content, faction, seed) {
            Err(error) => {
                failures += 1;
                eprintln!("FAIL faction={faction} seed={seed}: {error}");
            }
            Ok(first) => match play(content, faction, seed) {
                Err(error) => {
                    failures += 1;
                    eprintln!("FAIL faction={faction} seed={seed} replay: {error}");
                }
                Ok(second) if first != second => {
                    failures += 1;
                    let divergence = first
                        .decisions
                        .iter()
                        .zip(&second.decisions)
                        .position(|(left, right)| left != right)
                        .unwrap_or_else(|| first.decisions.len().min(second.decisions.len()));
                    let prior = divergence
                        .checked_sub(1)
                        .and_then(|index| first.decisions.get(index))
                        .map_or_else(|| "<none>".to_owned(), |record| format!("{record:?}"));
                    eprintln!(
                        "FAIL faction={faction} seed={seed}: replay differs at decision {divergence}; prior={prior}; first={:?}; replay={:?}; events_equal={} state_equal={}",
                        first.decisions.get(divergence),
                        second.decisions.get(divergence),
                        first.events == second.events,
                        first.state == second.state
                    );
                }
                Ok(_) => {}
            },
        }
        if (seed - start + 1) % 25 == 0 {
            println!(
                "progress faction={faction} games={} failures={failures}",
                seed - start + 1
            );
            let _ = io::stdout().flush();
        }
    }
    println!("result faction={faction} games={count} failures={failures}");
    failures
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let chosen: Vec<&str> = match args.first() {
        Some(faction) if FACTIONS.contains(&faction.as_str()) => vec![faction],
        Some(faction) => {
            eprintln!(
                "unknown faction {faction}; expected one of {}",
                FACTIONS.join(", ")
            );
            std::process::exit(2);
        }
        None => FACTIONS.to_vec(),
    };
    let parse = |index: usize, default: u64| -> u64 {
        args.get(index).map_or(default, |value| {
            value.parse::<u64>().unwrap_or_else(|_| {
                eprintln!("argument {} must be a non-negative integer", index + 1);
                std::process::exit(2);
            })
        })
    };
    let start = parse(1, 0);
    let count = parse(2, 200);
    if count == 0 || start.checked_add(count).is_none() {
        eprintln!("count must be positive and the seed range must not overflow");
        std::process::exit(2);
    }
    let full_campaign = args.is_empty();
    let content = ContentStore::embedded();
    let failures: BTreeMap<&str, usize> = chosen
        .iter()
        .map(|faction| (*faction, run_faction(content, faction, start, count)))
        .collect();
    let total: usize = failures.values().sum();
    println!(
        "{}: factions={} games={} failures={total}",
        if full_campaign {
            "FULL 12x200 SOAK"
        } else {
            "diagnostic slice (not full acceptance)"
        },
        chosen.len(),
        chosen.len() as u64 * count
    );
    if total != 0 {
        std::process::exit(1);
    }
}
