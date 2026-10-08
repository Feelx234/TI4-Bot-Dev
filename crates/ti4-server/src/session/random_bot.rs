//! The in-process random bot: a seat answered inside the game worker by a decider that picks
//! among the engine's legal options.
//!
//! It is the server-side counterpart of the nightly browser harness's `random` policy
//! (`web/e2e/smokePlaythrough.ts`, `smokePolicy.ts`) without the browser. The harness clicks a
//! random enabled control and relies on helpers to finish staged workflows (confirm a complete
//! payment, confirm hits, confirm command tokens, propose a listed trade, end the turn). The
//! engine already collapses each of those workflows into one question whose options are the
//! legal answers, so a pick among the options is the same behaviour one level down. The
//! harness's special cases that the option list still needs are mirrored in [`weight`].
//!
//! Determinism: every pick is seeded from (game seed, seat id, decision index), where the
//! decision index is the number of decisions answered before this one in the whole game
//! (replayed ones included). The same position therefore gets the same answer, in the same
//! process or after a restart. A bot only ever *produces* new decisions; a replay or recovery
//! feeds the recorded answers through the replaying layer above this decider and never reaches it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_model::id::PlayerId;

use crate::session::worker::SessionShared;

/// Environment variable with the think delay between bot decisions, in milliseconds.
pub const RANDOM_BOT_DELAY_ENV: &str = "TI4_RANDOM_BOT_DELAY_MS";
/// Default think delay: long enough to follow along, short enough not to drag.
pub const DEFAULT_RANDOM_BOT_DELAY_MS: u64 = 600;
/// Hard cap so a typo cannot park a game for minutes per decision.
const MAX_DELAY_MS: u64 = 30_000;

/// The configured think delay: `TI4_RANDOM_BOT_DELAY_MS`, or 600 ms. `0` disables it.
#[must_use]
pub fn random_bot_delay_ms() -> u64 {
    std::env::var(RANDOM_BOT_DELAY_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RANDOM_BOT_DELAY_MS)
        .min(MAX_DELAY_MS)
}

fn mix(mut x: u64) -> u64 {
    // splitmix64 finaliser
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The RNG for one decision: a pure function of (game seed, seat, decision index).
#[must_use]
pub fn decision_rng(game_seed: u64, seat: &PlayerId, index: usize) -> ChaCha8Rng {
    let seed = mix(game_seed ^ mix(fnv(seat.as_str())) ^ mix(index as u64 ^ 0xA5A5_5A5A_0000_0000));
    ChaCha8Rng::seed_from_u64(seed)
}

/// Relative likelihood of one option. The harness `random` policy is uniform over the enabled
/// controls, so anything unmatched weighs 1. The exception mirrors its loop guard: opening a
/// transaction (its `trade-opt-` / `propose-trade-btn` controls weigh 0.1 in the steering table)
/// is rarely worth it and, picked as often as a move, drags a table into endless bargaining.
#[must_use]
pub fn weight(option: &ChoiceOption) -> f64 {
    if option.kind == ti4_engine::transactions::OPEN_KIND {
        0.1
    } else {
        1.0
    }
}

/// Pick one option index by weight with `rng`. The choice must have at least one option.
#[must_use]
pub fn pick_index(choice: &Choice, rng: &mut ChaCha8Rng) -> usize {
    let weights: Vec<f64> = choice.options.iter().map(weight).collect();
    let total: f64 = weights.iter().sum();
    let mut roll = rng.random_range(0.0..total);
    for (index, w) in weights.iter().enumerate() {
        if roll < *w {
            return index;
        }
        roll -= w;
    }
    weights.len() - 1
}

/// Shared count of decisions answered so far in this game, replayed ones included. Advanced by
/// [`CountingDecider`], read by [`RandomBotDecider`] as the decision index.
pub type DecisionCounter = Arc<AtomicUsize>;

/// Counts every answered decision of the seat it wraps (placed outside the replaying layer, so
/// recorded answers count too).
pub struct CountingDecider {
    pub inner: Box<dyn Decider>,
    pub counter: DecisionCounter,
}

impl Decider for CountingDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let option = self.inner.choose(choice)?;
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(option)
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let option = self.inner.choose_seeing(choice, seen)?;
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(option)
    }
}

/// The decider of a `BotRandom` seat.
pub struct RandomBotDecider {
    seat: PlayerId,
    game_seed: u64,
    counter: DecisionCounter,
    delay_ms: u64,
    shared: Arc<Mutex<SessionShared>>,
}

impl RandomBotDecider {
    #[must_use]
    pub fn new(
        seat: PlayerId,
        game_seed: u64,
        counter: DecisionCounter,
        delay_ms: u64,
        shared: Arc<Mutex<SessionShared>>,
    ) -> Self {
        Self {
            seat,
            game_seed,
            counter,
            delay_ms,
            shared,
        }
    }

    /// Wait out the think time in short slices without holding the session lock, so a stop
    /// (undo, redo, shutdown) is noticed within a slice. `Err` when the session was stopped.
    fn think(&self, rng: &mut ChaCha8Rng) -> Result<(), ()> {
        if self.delay_ms == 0 {
            return Ok(());
        }
        let jitter = rng.random_range(0.6..1.4);
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut remaining = (self.delay_ms as f64 * jitter) as u64;
        while remaining > 0 {
            if self.shared.lock().expect("shared lock").stopped {
                return Err(());
            }
            let slice = remaining.min(20);
            std::thread::sleep(Duration::from_millis(slice));
            remaining -= slice;
        }
        Ok(())
    }
}

impl Decider for RandomBotDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if choice.options.is_empty() {
            return Err(IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            });
        }
        let index = self.counter.load(Ordering::SeqCst);
        let mut rng = decision_rng(self.game_seed, &self.seat, index);
        let picked = pick_index(choice, &mut rng);
        if self.think(&mut rng).is_err() {
            return Err(IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "session was stopped".to_owned(),
            });
        }
        Ok(choice.options[picked].clone())
    }
}
