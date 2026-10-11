//! A survey of whole smart-bot games, for tuning the heuristics: `cargo test --test
//! smart_bot_survey -- --ignored --nocapture`. Plays a fixed set of tables (all-smart 3, 4 and 6
//! seats, and smart against random) and prints per-round standings and a summary per seat kind.
//! Set `SURVEY_DUMP=<dir>` to also write each game's decision log as JSON lines, and
//! `SURVEY_SEEDS=1,2,3` to play all-smart 3, 4 and 6-seat tables on those seeds instead, and
//! `SURVEY_MIXED_SEEDS=1,2,3` to choose the smart-against-random tables' seeds.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_server::session::{GameSession, SeatController, SessionConfig};

fn config_with(id: &str, seed: u64, controllers: &[SeatController]) -> SessionConfig {
    let ids: Vec<PlayerId> = (1..=controllers.len())
        .map(|i| PlayerId::new(format!("p{i}")))
        .collect();
    let (state, galaxy) = ti4_server::map::create_game_with_options(
        ContentStore::embedded(),
        &ids,
        seed,
        None,
        None,
        Some("te"),
    )
    .expect("create game");
    let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let mut config = SessionConfig::new(id, state)
        .with_seed(seed)
        .with_player_ids(ids.clone())
        .with_galaxy(galaxy, tiles)
        .with_random_bot_delay_ms(0);
    for (id, controller) in ids.into_iter().zip(controllers) {
        config = config.with_seat(id, controller.clone());
    }
    config
}

fn line(state: &GameState) -> String {
    let mut out = format!("  r{} {:?}:", state.round, state.phase);
    for seat in &state.players {
        let planets = state.controlled_planets(&seat.id).len();
        let ships = state
            .board
            .values()
            .flat_map(|s| s.units.iter())
            .filter(|u| u.owner == seat.id)
            .count();
        // Ground forces anywhere (space and planets), by id: infantry and mechs of every faction.
        let ground = state
            .board
            .values()
            .flat_map(|s| s.units.iter().chain(s.planet_units.values().flatten()))
            .filter(|u| u.owner == seat.id)
            .filter(|u| {
                let id = u.type_id.as_str();
                id.contains("infantry") || id.contains("mech")
            })
            .count();
        let _ = write!(
            out,
            " {}[{}vp {}pl {}u {}g tg{} t{}/f{}/s{} sc{:?}]",
            seat.id,
            seat.victory_points,
            planets,
            ships,
            ground,
            seat.trade_goods,
            seat.tactic_tokens,
            seat.fleet_tokens,
            seat.strategic_tokens,
            seat.strategy_cards
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
    }
    out
}

fn survey(id: &str, seed: u64, seats: &[SeatController], max_round: u32) -> GameState {
    let session = GameSession::start(config_with(id, seed, seats));
    let start = Instant::now();
    let mut round = 0;
    let mut last_len = 0;
    let mut last_progress = Instant::now();
    loop {
        let state = session.current_state();
        if state.round != round {
            round = state.round;
            eprintln!("{}", line(&state));
        }
        let len = session.decision_log().len();
        if len != last_len {
            last_len = len;
            last_progress = Instant::now();
        }
        if session.is_finished()
            || session.error().is_some()
            || state.round > max_round
            || last_progress.elapsed() > Duration::from_secs(60)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    session.stop();
    let state = session.current_state();
    eprintln!("{}", line(&state));
    eprintln!(
        "{id}: seed {seed} finished={} error={:?} decisions={} in {:.0?}",
        session.is_finished(),
        session.error(),
        session.decision_log().len(),
        start.elapsed()
    );
    if let Ok(dir) = std::env::var("SURVEY_DUMP") {
        let mut text = String::new();
        for record in session.decision_log() {
            text.push_str(&serde_json::to_string(&record).expect("json"));
            text.push('\n');
        }
        std::fs::write(format!("{dir}/{id}.jsonl"), text).expect("write dump");
        std::fs::write(
            format!("{dir}/{id}.state.json"),
            serde_json::to_string(&state).expect("json"),
        )
        .expect("write state");
    }
    state
}

#[test]
#[ignore = "a long survey for tuning, not a check"]
fn survey_smart_bot_games() {
    let only = std::env::var("SURVEY_ONLY").ok();
    let max_round: u32 = std::env::var("SURVEY_ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(12);
    let smart = SeatController::BotSmart;
    let random = SeatController::BotRandom;
    let mut tables: Vec<(String, u64, Vec<SeatController>)> = Vec::new();
    let seeds: Option<Vec<u64>> = std::env::var("SURVEY_SEEDS").ok().map(|list| {
        list.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    });
    let all_smart: Vec<(usize, u64)> = seeds.as_ref().map_or_else(
        || vec![(3, 5), (3, 23), (4, 7), (4, 31), (6, 13), (6, 47)],
        |seeds| {
            seeds
                .iter()
                .flat_map(|&seed| [3, 4, 6].map(|n| (n, seed)))
                .collect()
        },
    );
    for (n, seed) in all_smart {
        tables.push((format!("smart{n}_{seed}"), seed, vec![smart.clone(); n]));
    }
    let mixed_seeds: Vec<u64> = std::env::var("SURVEY_MIXED_SEEDS").ok().map_or_else(
        || {
            if seeds.is_some() {
                vec![]
            } else {
                vec![3, 17, 29]
            }
        },
        |list| {
            list.split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        },
    );
    for seed in mixed_seeds {
        for swap in [false, true] {
            let seats: Vec<SeatController> = (0..4)
                .map(|i| {
                    if (i % 2 == 0) == swap {
                        random.clone()
                    } else {
                        smart.clone()
                    }
                })
                .collect();
            tables.push((format!("mixed_{seed}_{swap}"), seed, seats));
        }
    }
    let (mut smart_vp, mut random_vp, mut smart_seats, mut random_seats) = (0, 0, 0, 0);
    let (mut smart_wins, mut random_wins) = (0, 0);
    let (mut games, mut to_ten, mut end_rounds) = (0, 0, 0);
    for (id, seed, seats) in tables {
        if only.as_deref().is_some_and(|o| !id.starts_with(o)) {
            continue;
        }
        let state = survey(&id, seed, &seats, max_round);
        let best = state
            .players
            .iter()
            .map(|p| p.victory_points)
            .max()
            .unwrap_or(0);
        games += 1;
        to_ten += usize::from(best >= 10);
        end_rounds += state.round;
        eprintln!("RESULT {id} round {} best {best}", state.round);
        for (seat, controller) in state.players.iter().zip(&seats) {
            let won = state.finished && seat.victory_points == best;
            if *controller == SeatController::BotSmart {
                smart_vp += seat.victory_points;
                smart_seats += 1;
                smart_wins += usize::from(won && id.starts_with("mixed"));
            } else {
                random_vp += seat.victory_points;
                random_seats += 1;
                random_wins += usize::from(won);
            }
        }
    }
    eprintln!("GAMES {games}: {to_ten} reached 10 VP, rounds played {end_rounds}");
    eprintln!(
        "SUMMARY smart {smart_vp} VP over {smart_seats} seats ({smart_wins} mixed wins); random {random_vp} VP over {random_seats} seats ({random_wins} wins)"
    );
}
