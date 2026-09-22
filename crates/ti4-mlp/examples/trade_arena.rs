//! Trade arena: pretrain the diplomacy head on single negotiations scored by heuristic values.
//!
//! `plans/TRADE_ARENA_VALUE_SHEET_2026-09-22.md`. A *vehicle* game is played by the current policy
//! with diplomacy on, only to supply real positions. Whenever a seat is offered diplomatic contact,
//! the arena may force one with a random partner (biased toward seats it can trade physical items
//! with); the seat's own policy never opens contact otherwise. The negotiation that follows is
//! played by the policy on both sides, then scored with `ti4_training::trade_arena`: each seat's
//! diplomacy decisions in it get that seat's score as their return. Nothing else is trained.
//!
//! Two bots share one actor per seat: a plain one plays the vehicle game, a recording one answers
//! the negotiation, so only arena decisions are ever recorded. The critic is untouched (PPO runs in
//! batch-mean mode); checkpoints keep the bundle's own value head.
//!
//! ```text
//! cargo run --release -p ti4-mlp --example trade_arena -- --bundle <dir> --out <dir> \
//!     [--updates 200] [--games-per-update 24] [--temperature 1.0] [--learning-rate 1e-4] \
//!     [--checkpoint-every 25] [--seed-base 1263000000] [--force 0.5] [--partner-bias 0.5] \
//!     [--device cuda] [--no-checkpoint]
//! ```

use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc, time::Instant};
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_engine::diplomacy::candidates::contact_seat_index;
use ti4_mlp::bundle::CriticMode;
use ti4_model::{
    DealRevision, DealTerm, DiplomacyEvent, PlayerId, TransferAsset, content_types::DEFAULT,
    id::FactionId,
};
use ti4_training::trade_arena;

const FACTIONS: [&str; 6] = ["sol", "letnev", "xxcha", "hacan", "jolnar", "l1z1x"];
/// Decision subtypes that belong to a negotiation window.
const NEGOTIATION: [&str; 5] = [
    "diplomacy_offer_item",
    "diplomacy_ask_item",
    "diplomacy_amount",
    "diplomacy_review",
    "diplomacy_response",
];
/// Map seeds are drawn this far from the run's seed so tiles and dice come from separate streams,
/// as in the PPO trainer.
const TILE_SEED_OFFSET: u64 = 20_000_000;
const ROUNDS: u32 = 4;

fn argument(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn refuse(reason: &str) -> ! {
    eprintln!("\nREFUSED: {reason}");
    std::process::exit(2);
}

fn parsed<T: std::str::FromStr>(name: &str, default: T) -> T {
    argument(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| refuse(&format!("{name}: '{value}' does not parse")))
    })
}

/// The negotiation the arena opened and has not scored yet.
#[derive(Debug, Clone)]
struct Open {
    proposer: PlayerId,
    recipient: PlayerId,
}

/// What the arena seats and the game loop share inside one game.
#[derive(Default)]
struct Shared {
    seating: Vec<PlayerId>,
    /// Seats each player can trade physical items with, refreshed at every turn change.
    partners: BTreeMap<PlayerId, Vec<PlayerId>>,
    /// The negotiation in progress.
    open: Option<Open>,
    /// A negotiation opened during the last step; the loop snapshots the position after it.
    opened: bool,
    /// A negotiation that ended during the last step; the loop scores it.
    closed: Option<Open>,
}

struct ArenaSeat {
    seat: PlayerId,
    vehicle: Box<dyn Decider>,
    arena: Box<dyn Decider>,
    shared: Rc<RefCell<Shared>>,
    rng: rand_chacha::ChaCha8Rng,
    force: f64,
    partner_bias: f64,
}

enum Route {
    Arena,
    Vehicle(Option<Choice>),
    Contact(ChoiceOption),
}

impl ArenaSeat {
    fn route(&mut self, choice: &Choice) -> Route {
        let subtype = choice
            .context
            .as_ref()
            .map_or("", |context| context.subtype.as_str());
        let negotiating = NEGOTIATION.contains(&subtype);
        let mut shared = self.shared.borrow_mut();
        if !negotiating && let Some(open) = shared.open.take() {
            shared.closed = Some(open);
        }
        if negotiating {
            return if shared.open.is_some() {
                Route::Arena
            } else {
                // A window the arena did not open (none should exist): played, not trained.
                Route::Vehicle(None)
            };
        }
        let contacts: Vec<&ChoiceOption> = choice
            .options
            .iter()
            .filter(|option| contact_seat_index(&option.id).is_some())
            .collect();
        if contacts.is_empty() {
            return Route::Vehicle(None);
        }
        if shared.open.is_none() && self.rng.random::<f64>() < self.force {
            let target_of = |option: &&ChoiceOption| {
                contact_seat_index(&option.id).and_then(|index| shared.seating.get(index).cloned())
            };
            let partners = shared.partners.get(&self.seat).cloned().unwrap_or_default();
            let trading: Vec<&ChoiceOption> = contacts
                .iter()
                .copied()
                .filter(|option| target_of(option).is_some_and(|t| partners.contains(&t)))
                .collect();
            let pool = if !trading.is_empty() && self.rng.random::<f64>() < self.partner_bias {
                trading
            } else {
                contacts
            };
            let pick = pool[self.rng.random_range(0..pool.len())];
            if let Some(recipient) = target_of(&pick) {
                shared.open = Some(Open {
                    proposer: self.seat.clone(),
                    recipient,
                });
                shared.opened = true;
                return Route::Contact(pick.clone());
            }
        }
        // The policy plays on, but never opens contact itself.
        let mut stripped = choice.clone();
        stripped
            .options
            .retain(|option| contact_seat_index(&option.id).is_none());
        if stripped.options.is_empty() {
            Route::Vehicle(None)
        } else {
            Route::Vehicle(Some(stripped))
        }
    }
}

impl Decider for ArenaSeat {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        match self.route(choice) {
            Route::Arena => self.arena.choose(choice),
            Route::Contact(option) => Ok(option),
            Route::Vehicle(stripped) => self.vehicle.choose(stripped.as_ref().unwrap_or(choice)),
        }
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        match self.route(choice) {
            Route::Arena => self.arena.choose_seeing(choice, seen),
            Route::Contact(option) => Ok(option),
            Route::Vehicle(stripped) => self
                .vehicle
                .choose_seeing(stripped.as_ref().unwrap_or(choice), seen),
        }
    }
}

/// One scored negotiation, for the report.
#[derive(Debug, Default, Clone)]
struct Episode {
    accepted: bool,
    offered: bool,
    counters: u32,
    trading_partner: bool,
    proposer_score: f64,
    recipient_score: f64,
    items: BTreeMap<&'static str, u32>,
}

/// A negotiation in flight: the position it opened in and where each seat's records started.
struct Pending {
    open: Open,
    state: ti4_model::GameState,
    journal_start: usize,
    starts: BTreeMap<PlayerId, usize>,
    trading_partner: bool,
}

fn kind_of(term: &DealTerm) -> &'static str {
    match term {
        DealTerm::ImmediateTransfer(asset) => match asset {
            TransferAsset::TradeGoods(_) => "trade goods",
            TransferAsset::Commodities(_) => "commodities",
            TransferAsset::CulturalFragments(_)
            | TransferAsset::HazardousFragments(_)
            | TransferAsset::IndustrialFragments(_)
            | TransferAsset::UnknownFragments(_) => "fragments",
            TransferAsset::PromissoryNote(note)
                if note.starts_with(ti4_engine::promissory::SUPPORT_PREFIX) =>
            {
                "support"
            }
            TransferAsset::PromissoryNote(_) => "other notes",
            TransferAsset::ActionCard(_) => "action cards",
            TransferAsset::SecretObjective(_) => "secrets",
        },
        _ => "promises",
    }
}

type Records = Rc<RefCell<Vec<ti4_mlp::bot::PpoRecord>>>;

/// Score a finished negotiation and write its return onto both seats' records.
fn settle(
    pending: Pending,
    game: &ti4_engine::game::Game<'_>,
    content: &ContentStore,
    handles: &BTreeMap<PlayerId, Records>,
    returns: &mut BTreeMap<PlayerId, Vec<Option<f64>>>,
) -> Episode {
    let mut latest: Option<DealRevision> = None;
    let mut episode = Episode {
        trading_partner: pending.trading_partner,
        ..Episode::default()
    };
    for entry in &game.state.diplomacy.journal[pending.journal_start..] {
        match &entry.event {
            DiplomacyEvent::Offered { revision, .. } => {
                episode.offered = true;
                latest = Some(revision.clone());
            }
            DiplomacyEvent::Countered { revision, .. } => {
                episode.counters += 1;
                latest = Some(revision.clone());
            }
            DiplomacyEvent::Accepted { .. } => episode.accepted = true,
            _ => {}
        }
    }
    let score = match (&latest, episode.accepted) {
        (Some(revision), true) => {
            for term in revision
                .proposer_terms
                .iter()
                .chain(&revision.recipient_terms)
            {
                *episode.items.entry(kind_of(term)).or_default() += 1;
            }
            trade_arena::score(
                &pending.state,
                content,
                &pending.open.proposer,
                &pending.open.recipient,
                revision,
            )
        }
        _ => trade_arena::DealScore::default(),
    };
    episode.proposer_score = score.proposer_score;
    episode.recipient_score = score.recipient_score;
    for (seat, value) in [
        (&pending.open.proposer, score.proposer_score),
        (&pending.open.recipient, score.recipient_score),
    ] {
        let end = handles[seat].borrow().len();
        let slots = returns.entry(seat.clone()).or_default();
        slots.resize(end, None);
        for slot in &mut slots[pending.starts[seat]..end] {
            *slot = Some(trade_arena::as_return(value));
        }
    }
    episode
}

struct GameOut {
    steps: Vec<ti4_mlp::ppo::Step>,
    episodes: Vec<Episode>,
    untracked: usize,
}

#[expect(clippy::too_many_arguments, reason = "a game's inputs")]
#[expect(clippy::too_many_lines, reason = "one game loop, read top to bottom")]
fn arena_game(
    actor: &Rc<ti4_mlp::Actor>,
    vocabulary: &ti4_policy::vocabulary::Vocabulary,
    pool: &Arc<ti4_sim::MapPool>,
    content: &ContentStore,
    seed: u64,
    temperature: f64,
    force: f64,
    partner_bias: f64,
) -> Result<GameOut, String> {
    let players: Vec<PlayerId> = (0..6).map(|i| PlayerId::new(format!("seat{i}"))).collect();
    let factions: BTreeMap<PlayerId, FactionId> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.clone(),
                ti4_training::rollout::seated_faction(&FACTIONS.map(FactionId::new), seed, 0, i),
            )
        })
        .collect();
    let shared = Rc::new(RefCell::new(Shared {
        seating: Vec::new(),
        ..Shared::default()
    }));
    let mut handles: BTreeMap<PlayerId, Records> = BTreeMap::new();
    let mut statuses = Vec::new();
    let mut game = ti4_training::rollout::setup_game_with_capabilities_and_decider_factory(
        content,
        &players,
        &factions,
        DEFAULT,
        seed,
        &ti4_training::rollout::OpeningMap::PythonPool {
            pool: Arc::clone(pool),
            tile_seed_offset: TILE_SEED_OFFSET,
        },
        ti4_training::rollout::SimulationCapabilities { diplomacy: true },
        |baselines| {
            let mut deciders: BTreeMap<PlayerId, Box<dyn Decider>> = BTreeMap::new();
            for (index, player) in players.iter().enumerate() {
                let row = ti4_mlp::FactionRow::of(factions[player].as_str())
                    .map_err(|error| format!("{player}: {error}"))?;
                let baseline = baselines[player];
                let stream = seed
                    .wrapping_mul(1_000_003)
                    .wrapping_add(u64::try_from(index).unwrap_or(0));
                let (vehicle, status) =
                    ti4_mlp::bot::MlpBot::sharing(actor, vocabulary.clone(), row, stream)
                        .at_temperature(temperature)
                        .from_setup(baseline)
                        .seat();
                statuses.push(status);
                let arena = ti4_mlp::bot::MlpBot::sharing(actor, vocabulary.clone(), row, !stream)
                    .at_temperature(temperature)
                    .recording_ppo(CriticMode::BatchMean)
                    .from_setup(baseline);
                handles.insert(player.clone(), arena.ppo_records());
                let (arena, status) = arena.seat();
                statuses.push(status);
                deciders.insert(
                    player.clone(),
                    Box::new(ArenaSeat {
                        seat: player.clone(),
                        vehicle,
                        arena,
                        shared: Rc::clone(&shared),
                        rng: rand_chacha::ChaCha8Rng::seed_from_u64(stream ^ 0xA7E4A),
                        force,
                        partner_bias,
                    }),
                );
            }
            Ok(deciders)
        },
    )?;
    shared.borrow_mut().seating = game.state.seating_order.clone();

    let target = game.state.round + ROUNDS;
    let mut turn = None;
    let mut pending: Option<Pending> = None;
    let mut episodes = Vec::new();
    let mut returns: BTreeMap<PlayerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut steps = 0usize;
    while !game.state.finished && game.state.round < target && steps < 400_000 {
        let now = (game.state.round, game.state.active.clone());
        if turn.as_ref() != Some(&now) {
            turn = Some(now);
            if let Some(galaxy) = game.galaxy() {
                let partners = players
                    .iter()
                    .map(|p| {
                        (
                            p.clone(),
                            ti4_engine::transactions::partners(&game.state, content, galaxy, p),
                        )
                    })
                    .collect();
                shared.borrow_mut().partners = partners;
            }
        }
        let result = game.step();
        if let Some(error) = result.error {
            return Err(format!("seed {seed}: {error:?}"));
        }
        steps += 1;
        let (closed, opened) = {
            let mut shared = shared.borrow_mut();
            (
                shared.closed.take(),
                std::mem::take(&mut shared.opened)
                    .then(|| shared.open.clone())
                    .flatten(),
            )
        };
        if closed.is_some()
            && let Some(done) = pending.take()
        {
            episodes.push(settle(done, &game, content, &handles, &mut returns));
        }
        if let Some(open) = opened {
            let starts = [&open.proposer, &open.recipient]
                .into_iter()
                .map(|seat| (seat.clone(), handles[seat].borrow().len()))
                .collect();
            let trading_partner = shared
                .borrow()
                .partners
                .get(&open.proposer)
                .is_some_and(|partners| partners.contains(&open.recipient));
            pending = Some(Pending {
                journal_start: game.state.diplomacy.journal.len(),
                state: game.state.clone(),
                starts,
                open,
                trading_partner,
            });
        }
    }
    if let Some(done) = pending.take() {
        episodes.push(settle(done, &game, content, &handles, &mut returns));
    }
    for status in statuses {
        status.into_result().map_err(|error| error.to_string())?;
    }
    let mut out = Vec::new();
    let mut untracked = 0;
    for (seat, handle) in &handles {
        let slots = returns.remove(seat).unwrap_or_default();
        for (index, record) in handle.borrow_mut().drain(..).enumerate() {
            match slots.get(index).copied().flatten() {
                Some(value) => {
                    let mut step = record.step;
                    step.return_to_go = value;
                    out.push(step);
                }
                None => untracked += 1,
            }
        }
    }
    Ok(GameOut {
        steps: out,
        episodes,
        untracked,
    })
}

#[expect(clippy::too_many_lines, reason = "the driver, read top to bottom")]
fn main() {
    ti4_tensor::configure_deterministic(20_260_922).unwrap_or_else(|e| refuse(&e.to_string()));
    let bundle_path = argument("--bundle").unwrap_or_else(|| refuse("--bundle is required"));
    let no_checkpoint = flag("--no-checkpoint");
    let out = argument("--out");
    if out.is_none() && !no_checkpoint {
        refuse("--out is required unless --no-checkpoint");
    }
    let updates: usize = parsed("--updates", 200);
    let games: u64 = parsed("--games-per-update", 24);
    let temperature: f64 = parsed("--temperature", 1.0);
    let learning_rate: f64 = parsed("--learning-rate", 1e-4);
    let every: usize = parsed("--checkpoint-every", 25);
    let seed_base: u64 = parsed("--seed-base", 1_263_000_000);
    let force: f64 = parsed("--force", 0.5);
    let partner_bias: f64 = parsed("--partner-bias", 0.5);
    let device = match argument("--device").as_deref().unwrap_or("cuda") {
        "cuda" => ti4_tensor::Device::Cuda(0),
        "cpu" => ti4_tensor::Device::Cpu,
        other => refuse(&format!("--device {other}: expected cuda or cpu")),
    };

    let loaded = ti4_mlp::bundle::read(std::path::Path::new(&bundle_path))
        .unwrap_or_else(|error| refuse(&format!("reading {bundle_path}: {error}")));
    let vocabulary = loaded.vocabulary;
    let bundle_mode = loaded.critic_mode;
    let mut actor = loaded.actor;
    if !actor.head_names().contains(&"diplomacy") {
        refuse("the bundle has no diplomacy head");
    }
    let slots_text = std::fs::read_to_string(std::path::Path::new(&bundle_path).join("slots.json"))
        .unwrap_or_else(|error| refuse(&format!("reading slots.json: {error}")));
    let pool_path =
        argument("--map-pool").unwrap_or_else(|| "out/pools/full_np8_12_train.json".to_owned());
    let pool_bytes = ti4_sim::artifacts::read_and_verify_pool_role(
        std::path::Path::new(&pool_path),
        &[ti4_sim::artifacts::ArtifactRole::Train],
    )
    .unwrap_or_else(|error| refuse(&format!("{pool_path}: {error}")));
    let pool = Arc::new(
        ti4_sim::MapPool::from_reader(std::io::Cursor::new(&pool_bytes))
            .unwrap_or_else(|error| refuse(&format!("{pool_path}: {error}"))),
    );
    let content = ContentStore::embedded();

    let settings = ti4_mlp::ppo::Settings {
        learning_rate,
        ..ti4_mlp::ppo::Settings::default()
    };
    actor = actor.to_device(device);
    let mut optimizer = ti4_mlp::ppo::Adam::new(&mut actor, CriticMode::BatchMean, settings)
        .unwrap_or_else(|error| refuse(&format!("opening Adam: {error}")));

    println!("trade arena");
    println!("  bundle      {bundle_path}");
    println!(
        "  scoring     alpha {} | 1 VP = {} TG | support {} | returns in VP (TG / {})",
        trade_arena::ALPHA,
        trade_arena::TRADE_GOODS_PER_VP,
        trade_arena::SUPPORT_VALUE,
        trade_arena::TRADE_GOODS_PER_VP
    );
    println!(
        "  play        {games} games per update | temperature {temperature} | force {force} | partner bias {partner_bias}"
    );
    println!("  seeds       {seed_base}.. | learning rate {learning_rate} | critic untouched\n");

    let workers = rayon::current_num_threads().max(1);
    for update in 0..updates {
        let started = Instant::now();
        let inference = actor.inference_copy().to_device(ti4_tensor::Device::Cpu);
        let base = seed_base + games * update as u64;
        let seeds: Vec<u64> = (base..base + games).collect();
        let chunk = seeds.len().div_ceil(workers).max(1);
        let jobs: Vec<(ti4_mlp::Actor, Vec<u64>)> = seeds
            .chunks(chunk)
            .map(|seeds| (inference.inference_copy(), seeds.to_vec()))
            .collect();
        let results: Vec<Result<GameOut, String>> = jobs
            .into_par_iter()
            .flat_map_iter(|(copy, seeds)| {
                let copy = Rc::new(copy);
                seeds
                    .into_iter()
                    .map(|seed| {
                        arena_game(
                            &copy,
                            &vocabulary,
                            &pool,
                            content,
                            seed,
                            temperature,
                            force,
                            partner_bias,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut steps = Vec::new();
        let mut episodes = Vec::new();
        let mut untracked = 0;
        for result in results {
            let game = result.unwrap_or_else(|error| refuse(&error));
            steps.extend(game.steps);
            episodes.extend(game.episodes);
            untracked += game.untracked;
        }
        let rollout = started.elapsed();
        if steps.is_empty() {
            refuse("an update produced no arena decisions");
        }
        let decisions = steps.len();
        let batch = ti4_mlp::ppo::Batch::freeze(steps, CriticMode::BatchMean)
            .unwrap_or_else(|error| refuse(&format!("freezing the batch: {error}")));
        let optimised = Instant::now();
        let stats = ti4_mlp::ppo::update(
            &mut actor,
            &batch,
            CriticMode::BatchMean,
            settings,
            seed_base ^ update as u64,
            &mut optimizer,
        )
        .unwrap_or_else(|error| refuse(&format!("update: {error}")));
        drop(batch);
        if matches!(device, ti4_tensor::Device::Cuda(_)) {
            ti4_tensor::cuda_cache::release_cached();
        }
        let optimise = optimised.elapsed();
        report(update, &episodes, decisions, untracked, rollout, optimise);
        if let Some(last) = stats.last() {
            println!(
                "              actor loss {:>9.5}  |log r| {:>7.5}  clipped {:>5.2}%  diplomacy entropy {:.4}",
                last.actor_loss,
                last.kl,
                last.clipped_fraction * 100.0,
                last.entropy.get("diplomacy").copied().unwrap_or(f64::NAN)
            );
        }
        let done = update + 1;
        if !no_checkpoint && (done % every == 0 || done == updates) {
            let destination = std::path::Path::new(out.as_deref().unwrap_or_default())
                .join(format!("checkpoint-{done}"));
            let cpu = actor.inference_copy().to_device(ti4_tensor::Device::Cpu);
            let written = ti4_mlp::bundle::write(
                &destination,
                &cpu,
                &slots_text,
                bundle_mode,
                &ti4_mlp::bundle::Provenance {
                    source: format!("trade arena, {done} update(s) from {bundle_path}"),
                    git_commit: std::env::var("GIT_COMMIT")
                        .unwrap_or_else(|_| "unrecorded".to_owned()),
                    update: done as u64,
                },
            )
            .unwrap_or_else(|error| refuse(&format!("writing the checkpoint: {error}")));
            ti4_mlp::bundle::read(&written.directory)
                .unwrap_or_else(|error| refuse(&format!("the checkpoint does not load: {error}")));
            println!("  checkpoint  {}", written.directory.display());
        }
    }
}

#[expect(clippy::cast_precision_loss, reason = "report counts are small")]
fn report(
    update: usize,
    episodes: &[Episode],
    decisions: usize,
    untracked: usize,
    rollout: std::time::Duration,
    optimise: std::time::Duration,
) {
    let n = episodes.len().max(1) as f64;
    let offered = episodes.iter().filter(|e| e.offered).count();
    let accepted: Vec<&Episode> = episodes.iter().filter(|e| e.accepted).collect();
    let a = accepted.len().max(1) as f64;
    let mean = |f: &dyn Fn(&Episode) -> f64| accepted.iter().map(|e| f(e)).sum::<f64>() / a;
    let partner = episodes.iter().filter(|e| e.trading_partner).count();
    let mut items: BTreeMap<&str, u32> = BTreeMap::new();
    for episode in &accepted {
        for (kind, count) in &episode.items {
            *items.entry(kind).or_default() += count;
        }
    }
    println!(
        "  update {update:>4}  negotiations {:>5} ({:.0}% trading partners)  offered {:.0}%  accepted {:.0}%  counters/offer {:.2}  decisions {decisions}  untracked {untracked}  rollout {:.1?}  optimise {:.1?}",
        episodes.len(),
        partner as f64 / n * 100.0,
        offered as f64 / n * 100.0,
        accepted.len() as f64 / n * 100.0,
        episodes.iter().map(|e| f64::from(e.counters)).sum::<f64>() / offered.max(1) as f64,
        rollout,
        optimise
    );
    println!(
        "              accepted deals: proposer {:+.2} TG  recipient {:+.2} TG  | items per deal: {}",
        mean(&|e| e.proposer_score),
        mean(&|e| e.recipient_score),
        items
            .iter()
            .map(|(kind, count)| format!("{kind} {:.2}", f64::from(*count) / a))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
