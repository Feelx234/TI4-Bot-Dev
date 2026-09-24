//! Standalone HTTP and WebSocket server binary for Twilight Imperium 4 online multiplayer.

use std::sync::Arc;
use std::time::Duration;
use ti4_server::http::create_app;
use ti4_server::session::GameRegistry;
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

    let store = Arc::new(ti4_server::storage::FileGameStore::new(&data_dir)?);
    let registry = Arc::new(GameRegistry::new().with_store(store));

    // Print recovery results even when tracing has no RUST_LOG filter configured.
    let recovery = registry.recover_all_games_report()?;
    for gid in &recovery.recovered {
        info!("Durable storage: recovered '{gid}' from disk");
    }
    for failure in &recovery.failed {
        eprintln!(
            "Durable storage: could not recover '{}' ({}): {}",
            failure.game_id, failure.stage, failure.error
        );
    }
    println!(
        "Durable storage: recovered {} game(s); {} failed (saved files retained)",
        recovery.recovered.len(),
        recovery.failed.len()
    );

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
