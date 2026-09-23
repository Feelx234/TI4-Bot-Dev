//! Stateless WebSocket bot client backed by an external advisor service.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use thiserror::Error;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_server::map::GalaxyLayout;
use ti4_server::protocol::server::Choice;
use ti4_server::protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage, parse_server_message};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const DEFAULT_TEMPERATURE: f64 = 0.25;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_RECONNECTS: u32 = 3;

/// Configuration for one independently seated bot process.
#[derive(Debug, Clone)]
pub struct BotConfig {
    pub server: String,
    pub game_id: String,
    pub seat: PlayerId,
    pub seat_token: String,
    pub advisor: String,
    pub temperature: f64,
    pub sample_seed: Option<u64>,
    pub timeout: Duration,
    pub max_reconnects: u32,
}

impl BotConfig {
    /// Builds a configuration using the documented deterministic defaults.
    #[must_use]
    pub fn new(
        server: String,
        game_id: String,
        seat: PlayerId,
        seat_token: String,
        advisor: String,
    ) -> Self {
        Self {
            server,
            game_id,
            seat,
            seat_token,
            advisor,
            temperature: DEFAULT_TEMPERATURE,
            sample_seed: None,
            timeout: DEFAULT_TIMEOUT,
            max_reconnects: DEFAULT_MAX_RECONNECTS,
        }
    }

    fn validate(&self) -> Result<(), BotError> {
        if self.server.is_empty() || !self.server.starts_with("ws://") {
            return Err(BotError::Configuration(
                "server must be a ws:// URL".to_owned(),
            ));
        }
        if self.game_id.is_empty() || self.seat_token.is_empty() {
            return Err(BotError::Configuration(
                "game and token must be non-empty".to_owned(),
            ));
        }
        if self.advisor.is_empty() || !self.advisor.starts_with("http://") {
            return Err(BotError::Configuration(
                "advisor must be an http:// URL".to_owned(),
            ));
        }
        if !self.temperature.is_finite() || self.temperature <= 0.0 {
            return Err(BotError::Configuration(
                "temperature must be finite and positive".to_owned(),
            ));
        }
        if self.timeout.is_zero() {
            return Err(BotError::Configuration(
                "timeout must be positive".to_owned(),
            ));
        }
        Ok(())
    }

    fn websocket_url(&self) -> String {
        format!(
            "{}/ws/games/{}",
            self.server.trim_end_matches('/'),
            self.game_id
        )
    }

    fn advisor_url(&self) -> String {
        format!("{}/evaluate", self.advisor.trim_end_matches('/'))
    }
}

/// A terminal bot-agent failure. Errors always leave the server state untouched.
#[derive(Debug, Error)]
pub enum BotError {
    #[error("invalid configuration: {0}")]
    Configuration(String),
    #[error("websocket connection or I/O failed: {0}")]
    WebSocket(String),
    #[error("advisor request failed: {0}")]
    Advisor(String),
    #[error("server protocol failed: {0}")]
    Protocol(String),
    #[error("server rejected the bot protocol: {0}")]
    Server(String),
    #[error("reconnect attempts exhausted")]
    ReconnectExhausted,
}

#[derive(Debug, Deserialize)]
struct AdvisorResponse {
    options: Vec<AdvisorOption>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdvisorOption {
    option_id: String,
    probability: f64,
    logit: f64,
}

/// Runs until the game ends or a bounded transport/protocol failure occurs.
///
/// # Errors
///
/// Returns an error for invalid configuration, unreachable peers, malformed protocol data,
/// invalid advisor output, or exhausted reconnect attempts.
pub async fn run(config: BotConfig) -> Result<(), BotError> {
    config.validate()?;
    let client = reqwest::Client::builder()
        .timeout(config.timeout)
        .build()
        .map_err(|error| {
            BotError::Configuration(format!("cannot build advisor client: {error}"))
        })?;
    let mut sampler = config.sample_seed.map(ChaCha8Rng::seed_from_u64);

    for attempt in 0..=config.max_reconnects {
        match run_connection(&config, &client, sampler.as_mut()).await {
            Ok(ConnectionEnd::GameOver) => return Ok(()),
            Ok(ConnectionEnd::Disconnected) | Err(BotError::WebSocket(_))
                if attempt < config.max_reconnects =>
            {
                tokio::time::sleep(reconnect_delay(attempt)).await;
            }
            Ok(ConnectionEnd::Disconnected) | Err(BotError::WebSocket(_)) => {
                return Err(BotError::ReconnectExhausted);
            }
            Err(error) => return Err(error),
        }
    }
    Err(BotError::ReconnectExhausted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionEnd {
    GameOver,
    Disconnected,
}

async fn run_connection(
    config: &BotConfig,
    client: &reqwest::Client,
    mut sampler: Option<&mut ChaCha8Rng>,
) -> Result<ConnectionEnd, BotError> {
    let (mut stream, _) = within(config.timeout, connect_async(config.websocket_url()))
        .await
        .map_err(BotError::WebSocket)?
        .map_err(|error| BotError::WebSocket(error.to_string()))?;
    send_message(
        &mut stream,
        &ClientMessage::Subscribe {
            protocol_version: PROTOCOL_VERSION,
            game_id: config.game_id.clone(),
            seat_token: Some(config.seat_token.clone()),
        },
        config.timeout,
    )
    .await?;

    let mut last_submission: Option<(String, u64)> = None;
    loop {
        let incoming = within(config.timeout, stream.next())
            .await
            .map_err(BotError::WebSocket)?;
        let Some(message) = incoming else {
            return Ok(ConnectionEnd::Disconnected);
        };
        let message = message.map_err(|error| BotError::WebSocket(error.to_string()))?;
        let Message::Text(text) = message else {
            if message.is_close() {
                return Ok(ConnectionEnd::Disconnected);
            }
            continue;
        };
        let message =
            parse_server_message(&text).map_err(|error| BotError::Protocol(error.to_string()))?;
        if let Some(game_id) = message.game_id()
            && game_id != config.game_id
        {
            return Err(BotError::Protocol(
                "server message game_id differs from subscription".to_owned(),
            ));
        }
        match message {
            ServerMessage::GameOver(_) => return Ok(ConnectionEnd::GameOver),
            ServerMessage::Error(error) => return Err(BotError::Server(error.message)),
            // A rejected nonce/version remains remembered, so only a new pending message may act.
            ServerMessage::ActionRejected(_) if last_submission.is_none() => {
                return Err(BotError::Protocol(
                    "received action rejection without a bot submission".to_owned(),
                ));
            }
            ServerMessage::InitialSnapshot(snapshot) => {
                if let Some(pending) = snapshot.pending_choice {
                    submit_advice(
                        config,
                        client,
                        &mut stream,
                        &snapshot.state,
                        &snapshot.galaxy_layout,
                        &pending.choice,
                        &pending.nonce,
                        snapshot.game_version,
                        sampler.as_deref_mut(),
                        &mut last_submission,
                    )
                    .await?;
                }
            }
            ServerMessage::StateUpdate(update) => {
                if let Some(pending) = update.pending_choice {
                    submit_advice(
                        config,
                        client,
                        &mut stream,
                        &update.state,
                        &update.galaxy_layout,
                        &pending.choice,
                        &pending.nonce,
                        update.game_version,
                        sampler.as_deref_mut(),
                        &mut last_submission,
                    )
                    .await?;
                }
            }
            ServerMessage::PendingChoice(pending) => {
                submit_advice(
                    config,
                    client,
                    &mut stream,
                    &pending.state,
                    &pending.galaxy_layout,
                    &pending.choice,
                    &pending.nonce,
                    pending.game_version,
                    sampler.as_deref_mut(),
                    &mut last_submission,
                )
                .await?;
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn submit_advice<S>(
    config: &BotConfig,
    client: &reqwest::Client,
    stream: &mut S,
    state: &GameState,
    galaxy_layout: &GalaxyLayout,
    choice: &Choice,
    nonce: &str,
    game_version: u64,
    sampler: Option<&mut ChaCha8Rng>,
    last_submission: &mut Option<(String, u64)>,
) -> Result<(), BotError>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    if choice.player != config.seat {
        return Err(BotError::Protocol(
            "pending choice belongs to another seat".to_owned(),
        ));
    }
    let key = (nonce.to_owned(), game_version);
    if last_submission.as_ref() == Some(&key) {
        return Ok(());
    }
    let offered = offered_ids(choice)?;
    let response = client
        .post(config.advisor_url())
        .json(&serde_json::json!({
            "state": state,
            "galaxy_layout": galaxy_layout,
            "player": &config.seat,
            "choice": choice,
            "temperature": config.temperature,
        }))
        .send()
        .await
        .map_err(|error| BotError::Advisor(error.to_string()))?
        .error_for_status()
        .map_err(|error| BotError::Advisor(error.to_string()))?
        .json::<AdvisorResponse>()
        .await
        .map_err(|error| BotError::Advisor(error.to_string()))?;
    let probabilities = validate_advice(&offered, response)?;
    let option_id = select_option(&offered, &probabilities, sampler)?;
    send_message(
        stream,
        &ClientMessage::SubmitChoice {
            protocol_version: PROTOCOL_VERSION,
            game_id: config.game_id.clone(),
            nonce: nonce.to_owned(),
            expected_version: game_version,
            option_id,
        },
        config.timeout,
    )
    .await?;
    *last_submission = Some(key);
    Ok(())
}

fn offered_ids(choice: &Choice) -> Result<Vec<String>, BotError> {
    if choice.options.is_empty() {
        return Err(BotError::Protocol(
            "pending choice has no options".to_owned(),
        ));
    }
    let ids: Vec<_> = choice
        .options
        .iter()
        .map(|option| option.id.clone())
        .collect();
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(BotError::Protocol(
            "pending choice has duplicate option IDs".to_owned(),
        ));
    }
    Ok(ids)
}

fn validate_advice(
    offered: &[String],
    response: AdvisorResponse,
) -> Result<BTreeMap<String, f64>, BotError> {
    if response.options.len() != offered.len() {
        return Err(BotError::Advisor(
            "advisor response does not cover every offered option".to_owned(),
        ));
    }
    let offered: BTreeSet<_> = offered.iter().map(String::as_str).collect();
    let mut probabilities = BTreeMap::new();
    for option in response.options {
        if !offered.contains(option.option_id.as_str())
            || !option.probability.is_finite()
            || option.probability < 0.0
            || !option.logit.is_finite()
            || probabilities
                .insert(option.option_id, option.probability)
                .is_some()
        {
            return Err(BotError::Advisor(
                "advisor response is malformed".to_owned(),
            ));
        }
    }
    if probabilities.values().sum::<f64>() <= 0.0 {
        return Err(BotError::Advisor(
            "advisor response has zero probability mass".to_owned(),
        ));
    }
    Ok(probabilities)
}

fn select_option(
    offered: &[String],
    probabilities: &BTreeMap<String, f64>,
    sampler: Option<&mut ChaCha8Rng>,
) -> Result<String, BotError> {
    if let Some(rng) = sampler {
        let total = probabilities.values().sum::<f64>();
        let mut target = rng.random_range(0.0..total);
        for option_id in offered {
            target -= probabilities[option_id];
            if target <= 0.0 {
                return Ok(option_id.clone());
            }
        }
        return offered
            .last()
            .cloned()
            .ok_or_else(|| BotError::Advisor("advisor response has no options".to_owned()));
    }
    let first = offered
        .first()
        .ok_or_else(|| BotError::Advisor("advisor response has no options".to_owned()))?;
    let selected = offered.iter().skip(1).fold(first, |selected, candidate| {
        if probabilities[candidate] > probabilities[selected] {
            candidate
        } else {
            selected
        }
    });
    Ok(selected.clone())
}

async fn send_message<S>(
    stream: &mut S,
    message: &ClientMessage,
    timeout: Duration,
) -> Result<(), BotError>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let text = serde_json::to_string(message)
        .map_err(|error| BotError::Protocol(format!("cannot serialize client message: {error}")))?;
    within(timeout, stream.send(Message::Text(text.into())))
        .await
        .map_err(BotError::WebSocket)?
        .map_err(|error| BotError::WebSocket(error.to_string()))
}

async fn within<T, F>(timeout: Duration, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| "operation timed out".to_owned())
}

fn reconnect_delay(attempt: u32) -> Duration {
    Duration::from_millis(100 * (1_u64 << attempt.min(5)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use ti4_server::fixtures::{sample_actor_snapshot, sample_terminal_game_over};

    fn advice(options: &[(&str, f64, f64)]) -> AdvisorResponse {
        AdvisorResponse {
            options: options
                .iter()
                .map(|(option_id, probability, logit)| AdvisorOption {
                    option_id: (*option_id).to_owned(),
                    probability: *probability,
                    logit: *logit,
                })
                .collect(),
        }
    }

    #[test]
    fn argmax_selects_the_highest_advisor_probability() {
        let offered = vec!["first".to_owned(), "second".to_owned()];
        let probabilities = validate_advice(
            &offered,
            advice(&[("first", 0.25, 2.0), ("second", 0.75, 1.0)]),
        )
        .expect("valid advice");
        assert_eq!(
            select_option(&offered, &probabilities, None).unwrap(),
            "second"
        );
    }

    #[test]
    fn argmax_breaks_equal_probabilities_by_original_option_order() {
        let offered = vec!["first".to_owned(), "second".to_owned()];
        let probabilities = validate_advice(
            &offered,
            advice(&[("first", 0.5, 2.0), ("second", 0.5, 1.0)]),
        )
        .expect("valid advice");
        assert_eq!(
            select_option(&offered, &probabilities, None).unwrap(),
            "first"
        );
    }

    #[test]
    fn seeded_sampling_is_reproducible() {
        let offered = vec!["first".to_owned(), "second".to_owned()];
        let probabilities = validate_advice(
            &offered,
            advice(&[("first", 0.25, 0.0), ("second", 0.75, 1.0)]),
        )
        .expect("valid advice");
        let mut left = ChaCha8Rng::seed_from_u64(42);
        let mut right = ChaCha8Rng::seed_from_u64(42);
        let left: Vec<_> = (0..10)
            .map(|_| select_option(&offered, &probabilities, Some(&mut left)).unwrap())
            .collect();
        let right: Vec<_> = (0..10)
            .map(|_| select_option(&offered, &probabilities, Some(&mut right)).unwrap())
            .collect();
        assert_eq!(left, right);
    }

    #[test]
    fn malformed_advice_is_rejected_without_a_submission() {
        let offered = vec!["first".to_owned(), "second".to_owned()];
        assert!(
            validate_advice(
                &offered,
                advice(&[("first", 0.5, 0.0), ("unknown", 0.5, 0.0)])
            )
            .is_err()
        );
        assert!(
            validate_advice(
                &offered,
                advice(&[("first", 0.5, 0.0), ("first", 0.5, 0.0)])
            )
            .is_err()
        );
        assert!(
            validate_advice(
                &offered,
                advice(&[("first", f64::NAN, 0.0), ("second", 1.0, 0.0)])
            )
            .is_err()
        );
        assert!(
            validate_advice(
                &offered,
                advice(&[("first", 0.0, 0.0), ("second", 0.0, 0.0)])
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn initial_snapshot_submission_uses_its_legal_option_nonce_and_version() {
        let advisor_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind advisor");
        let advisor_address = advisor_listener.local_addr().expect("advisor address");
        let advisor = Router::new().route(
            "/evaluate",
            post(|| async {
                Json(serde_json::json!({
                    "options": [
                        { "option_id": "opt_carrier", "probability": 0.2, "logit": 1.0 },
                        { "option_id": "opt_infantry", "probability": 0.8, "logit": 2.0 },
                        { "option_id": "decline", "probability": 0.0, "logit": -1.0 }
                    ]
                }))
            }),
        );
        let advisor_task = tokio::spawn(async move {
            axum::serve(advisor_listener, advisor)
                .await
                .expect("serve advisor");
        });

        let websocket_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket");
        let websocket_address = websocket_listener.local_addr().expect("websocket address");
        let websocket_task = tokio::spawn(async move {
            let (stream, _) = websocket_listener.accept().await.expect("accept bot");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("upgrade websocket");
            let subscribe = socket
                .next()
                .await
                .expect("subscribe message")
                .expect("subscribe transport");
            let subscribe: ClientMessage =
                serde_json::from_str(subscribe.to_text().expect("text")).expect("parse subscribe");
            assert!(matches!(
                subscribe,
                ClientMessage::Subscribe { ref game_id, ref seat_token, .. }
                    if game_id == "game_12345" && seat_token.as_deref() == Some("seat-token")
            ));

            let snapshot =
                serde_json::to_string(&sample_actor_snapshot()).expect("serialize snapshot");
            socket
                .send(Message::Text(snapshot.into()))
                .await
                .expect("send snapshot");
            let submission = socket
                .next()
                .await
                .expect("submission message")
                .expect("submission transport");
            let submission: ClientMessage =
                serde_json::from_str(submission.to_text().expect("text"))
                    .expect("parse submission");
            assert!(matches!(
                submission,
                ClientMessage::SubmitChoice {
                    ref nonce,
                    expected_version: 42,
                    ref option_id,
                    ..
                } if nonce == "nonce_xyz789" && option_id == "opt_infantry"
            ));
            let terminal =
                serde_json::to_string(&sample_terminal_game_over()).expect("serialize terminal");
            socket
                .send(Message::Text(terminal.into()))
                .await
                .expect("send terminal");
        });

        let mut config = BotConfig::new(
            format!("ws://{websocket_address}"),
            "game_12345".to_owned(),
            PlayerId::new("seat_a"),
            "seat-token".to_owned(),
            format!("http://{advisor_address}"),
        );
        config.timeout = Duration::from_secs(2);
        config.max_reconnects = 0;
        tokio::time::timeout(Duration::from_secs(5), run(config))
            .await
            .expect("bot run should not hang")
            .expect("bot should finish at game over");
        websocket_task.await.expect("websocket task");
        advisor_task.abort();
    }

    #[tokio::test]
    async fn reconnect_uses_a_new_initial_snapshot_without_resending_a_cached_choice() {
        let advisor_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind advisor");
        let advisor_address = advisor_listener.local_addr().expect("advisor address");
        let advisor = Router::new().route(
            "/evaluate",
            post(|| async {
                Json(serde_json::json!({
                    "options": [
                        { "option_id": "opt_carrier", "probability": 1.0, "logit": 1.0 },
                        { "option_id": "opt_infantry", "probability": 0.0, "logit": 0.0 },
                        { "option_id": "decline", "probability": 0.0, "logit": -1.0 }
                    ]
                }))
            }),
        );
        let advisor_task = tokio::spawn(async move {
            axum::serve(advisor_listener, advisor)
                .await
                .expect("serve advisor");
        });

        let websocket_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket");
        let websocket_address = websocket_listener.local_addr().expect("websocket address");
        let websocket_task = tokio::spawn(async move {
            let (first_stream, _) = websocket_listener.accept().await.expect("first accept");
            let mut first = tokio_tungstenite::accept_async(first_stream)
                .await
                .expect("first upgrade");
            first
                .next()
                .await
                .expect("first subscribe")
                .expect("first transport");
            drop(first);

            let (second_stream, _) = websocket_listener.accept().await.expect("second accept");
            let mut second = tokio_tungstenite::accept_async(second_stream)
                .await
                .expect("second upgrade");
            second
                .next()
                .await
                .expect("second subscribe")
                .expect("second transport");
            let snapshot =
                serde_json::to_string(&sample_actor_snapshot()).expect("serialize snapshot");
            second
                .send(Message::Text(snapshot.into()))
                .await
                .expect("send snapshot");
            let submission = second
                .next()
                .await
                .expect("submission message")
                .expect("submission transport");
            let submission: ClientMessage =
                serde_json::from_str(submission.to_text().expect("text"))
                    .expect("parse submission");
            assert!(matches!(
                submission,
                ClientMessage::SubmitChoice { ref option_id, .. } if option_id == "opt_carrier"
            ));
            let terminal =
                serde_json::to_string(&sample_terminal_game_over()).expect("serialize terminal");
            second
                .send(Message::Text(terminal.into()))
                .await
                .expect("send terminal");
        });

        let mut config = BotConfig::new(
            format!("ws://{websocket_address}"),
            "game_12345".to_owned(),
            PlayerId::new("seat_a"),
            "seat-token".to_owned(),
            format!("http://{advisor_address}"),
        );
        config.timeout = Duration::from_secs(2);
        config.max_reconnects = 1;
        tokio::time::timeout(Duration::from_secs(5), run(config))
            .await
            .expect("bot run should not hang")
            .expect("bot should reconnect and finish");
        websocket_task.await.expect("websocket task");
        advisor_task.abort();
    }
}
