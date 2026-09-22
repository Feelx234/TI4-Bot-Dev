//! Standalone HTTP and WebSocket server binary for Twilight Imperium 4 online multiplayer.

use std::sync::Arc;
use std::time::Duration;

use ti4_content::ContentStore;
use ti4_model::id::PlayerId;
use ti4_server::http::create_app;
use ti4_server::session::{GameRegistry, SeatController, SessionConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let args: Vec<String> = std::env::args().collect();
    let mut data_dir = std::env::var("TI4_DATA_DIR").unwrap_or_else(|_| "./data/games".to_owned());
    for i in 0..args.len() {
        if args[i] == "--data-dir" && i + 1 < args.len() {
            data_dir = args[i + 1].clone();
        }
    }

    let lease_ms = std::env::var("TI4_SEAT_LEASE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(30))
        .clamp(Duration::from_secs(1), Duration::from_secs(300));
    let store = Arc::new(ti4_server::storage::FileGameStore::new(&data_dir)?);
    let registry = Arc::new(
        GameRegistry::new()
            .with_store(store)
            .with_lease_duration(lease_ms),
    );

    // Recover existing saved games from disk
    match registry.recover_all_games() {
        Ok(recovered) if !recovered.is_empty() => {
            for gid in &recovered {
                info!("Durable storage: recovered active session '{gid}' from disk");
            }
        }
        Ok(_) => {
            info!("Durable storage: initialized with no prior games");
        }
        Err(err) => {
            tracing::warn!("Durable storage recovery encountered an error: {err}");
        }
    }

    // Create a default demo game ("demo") if not already recovered from storage
    if registry.get_game("demo").is_none() {
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        let p3 = PlayerId::new("p3");
        let players = [p1.clone(), p2.clone(), p3.clone()];

        let content = ContentStore::embedded();
        if let Ok((state, galaxy)) = ti4_server::map::create_game_with_map(content, &players, 42) {
            let map_tiles = ti4_server::map::build_board_tiles(content, &galaxy);
            let config = SessionConfig::new("demo", state)
                .with_seed(42)
                .with_player_ids(players.to_vec())
                .with_galaxy(galaxy, map_tiles)
                .with_seat(p1, SeatController::Human)
                .with_seat(p2, SeatController::Human)
                .with_seat(p3, SeatController::BotFirstOption);

            if let Ok(session) = registry.create_game(config) {
                info!(
                    "Created default demo game with galaxy map: '{}'",
                    session.id()
                );
            }
        }
    } else {
        info!("Demo game 'demo' successfully resumed from disk storage");
    }

    let app = create_app(registry);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("authoritative server listening on http://{addr}");

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║  Twilight Imperium 4 — Authoritative Server                     ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Listening on:  http://127.0.0.1:{port:<29}║");
    println!(
        "║  Health Check:  http://127.0.0.1:{port}/health{pad:<22}║",
        pad = ""
    );
    println!("║  Default Game:  'demo' (Seats: p1 [Human], p2 [Human], p3 [Bot])║");
    println!("║  Storage Dir:   {data_dir:<48}║");
    println!("║                                                                  ║");
    println!("║  Web Client:    cd web && npm run dev                            ║");
    println!("║  Open in UI:    http://127.0.0.1:3000                            ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            info!("Received shutdown signal, terminating server");
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await?;

    Ok(())
}
