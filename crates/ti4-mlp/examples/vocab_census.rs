//! Which feature names today's engine produces that a checkpoint's vocabulary has never seen.
//!
//! A name the vocabulary does not know is routed to its family's out-of-vocabulary column, so two
//! new options that differ only in unseen names look identical to the policy. This plays games in
//! the training regime (diplomacy on, the trainer's seating and horizon), projects every decision
//! the way the MLP does, and counts the admitted names the bundle cannot place, by decision subtype.
//! The list it prints is the input to `migrate_bundle_append_names`.
//!
//! ```text
//! cargo run --release -p ti4-mlp --example vocab_census -- \
//!   --bundle <checkpoint> --map-pool out/pools/full_np8_12_train.json \
//!   --seeds 2 --rounds 4 --temperature 2.5 --min-count 3 --names-out <file>
//! ```

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, Decider};
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{FactionId, PlayerId};

const FACTIONS: [&str; 6] = ["sol", "letnev", "xxcha", "hacan", "jolnar", "l1z1x"];
const TILE_SEED_OFFSET: u64 = 7_000_000;

/// Unseen admitted names: name -> (count, a subtype it appeared under).
type Census = BTreeMap<String, (usize, String)>;

struct Watching {
    inner: Box<dyn Decider>,
    vocabulary: ti4_policy::vocabulary::Vocabulary,
    baseline: ti4_policy::progress::Baseline,
    census: Rc<RefCell<Census>>,
}

impl Decider for Watching {
    fn choose(
        &mut self,
        choice: &Choice,
    ) -> Result<ti4_engine::choice::ChoiceOption, ti4_engine::choice::IllegalChoice> {
        self.inner.choose(choice)
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &ti4_engine::choice::SeatObservation<'_>,
    ) -> Result<ti4_engine::choice::ChoiceOption, ti4_engine::choice::IllegalChoice> {
        let subtype = choice
            .context
            .as_ref()
            .map_or_else(|| "none".to_owned(), |context| context.subtype.clone());
        let vectors = ti4_policy::projection::mlp_choice_features(
            seen.observed(),
            choice,
            &choice.player,
            &seen.held_secret_progress(),
            self.baseline,
        );
        let mut census = self.census.borrow_mut();
        for vector in &vectors {
            for key in vector.keys() {
                if self.vocabulary.is_assigned_key(*key) {
                    continue;
                }
                let name = ti4_policy::intern::name_of(*key);
                if !ti4_policy::projection::admits(&name) {
                    continue;
                }
                let entry = census.entry(name).or_insert((0, subtype.clone()));
                entry.0 += 1;
            }
        }
        drop(census);
        self.inner.choose_seeing(choice, seen)
    }
}

fn argument(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1))
        .cloned()
}

fn refuse(reason: &str) -> ! {
    eprintln!("\nREFUSED: {reason}");
    std::process::exit(2)
}

fn number<T: std::str::FromStr>(name: &str, fallback: T) -> T {
    argument(name).map_or(fallback, |value| {
        value
            .parse()
            .unwrap_or_else(|_| refuse(&format!("{name} expects a number, got {value:?}")))
    })
}

fn main() {
    let bundle_path = argument("--bundle").unwrap_or_else(|| refuse("--bundle is required"));
    let pool_path = argument("--map-pool").unwrap_or_else(|| refuse("--map-pool is required"));
    let seeds: u64 = number("--seeds", 2);
    let seed_base: u64 = number("--seed-base", 1_263_000_000);
    let rounds: u32 = number("--rounds", 4);
    let temperature: f64 = number("--temperature", 2.5);
    let min_count: usize = number("--min-count", 3);

    ti4_tensor::configure_deterministic(20_260_922)
        .unwrap_or_else(|error| refuse(&format!("configuring the backend: {error}")));
    let content = ContentStore::embedded();
    let bundle = ti4_mlp::bundle::read(std::path::Path::new(&bundle_path))
        .unwrap_or_else(|error| refuse(&format!("reading {bundle_path}: {error}")));
    let vocabulary = bundle.vocabulary;
    let actor = Rc::new(bundle.actor.inference_copy());
    let pool = Arc::new(
        ti4_sim::MapPool::from_reader(std::io::Cursor::new(
            std::fs::read(&pool_path)
                .unwrap_or_else(|error| refuse(&format!("reading {pool_path}: {error}"))),
        ))
        .unwrap_or_else(|error| refuse(&format!("parsing the pool: {error}"))),
    );
    let players: Vec<PlayerId> = (0..6)
        .map(|index| PlayerId::new(format!("seat{index}")))
        .collect();
    let factions: [FactionId; 6] = FACTIONS.map(FactionId::new);
    let census: Rc<RefCell<Census>> = Rc::new(RefCell::new(Census::new()));
    let mut games = 0_usize;
    let mut errors = 0_usize;
    let mut decisions = 0_usize;
    let started = std::time::Instant::now();

    for seed in seed_base..seed_base + seeds {
        for rotation in 0..FACTIONS.len() {
            let seated: BTreeMap<PlayerId, FactionId> = players
                .iter()
                .enumerate()
                .map(|(index, player)| {
                    (
                        player.clone(),
                        ti4_training::rollout::seated_faction(&factions, seed, rotation, index),
                    )
                })
                .collect();
            let (rollout, _) =
                ti4_training::rollout::play_with_capabilities_and_decider_factory_digest(
                    content,
                    &players,
                    &seated,
                    DEFAULT,
                    seed,
                    ti4_training::rollout::Horizon {
                        rounds,
                        steps: 10_000,
                    },
                    ti4_engine::opening::DEFAULT_REQUIREMENT,
                    &ti4_training::rollout::OpeningMap::PythonPool {
                        pool: Arc::clone(&pool),
                        tile_seed_offset: TILE_SEED_OFFSET,
                    },
                    ti4_training::rollout::SimulationCapabilities { diplomacy: true },
                    false,
                    |baselines| {
                        let mut deciders: BTreeMap<PlayerId, Box<dyn Decider>> = BTreeMap::new();
                        for (index, player) in players.iter().enumerate() {
                            let row = ti4_mlp::FactionRow::of(seated[player].as_str())
                                .map_err(|error| format!("{player}: {error}"))?;
                            let baseline = baselines
                                .get(player)
                                .copied()
                                .ok_or_else(|| format!("{player} has no setup baseline"))?;
                            let stream = seed
                                .wrapping_mul(1_000_003)
                                .wrapping_add(u64::try_from(index).unwrap_or(0));
                            let bot = ti4_mlp::bot::MlpBot::sharing(
                                &actor,
                                vocabulary.clone(),
                                row,
                                stream,
                            )
                            .at_temperature(temperature)
                            .from_setup(baseline);
                            let (decider, _status) = bot.seat();
                            deciders.insert(
                                player.clone(),
                                Box::new(Watching {
                                    inner: decider,
                                    vocabulary: vocabulary.clone(),
                                    baseline,
                                    census: Rc::clone(&census),
                                }),
                            );
                        }
                        Ok(deciders)
                    },
                );
            games += 1;
            decisions += rollout
                .seats
                .iter()
                .map(|seat| seat.trajectory.len())
                .sum::<usize>();
            if let Some(error) = &rollout.error {
                errors += 1;
                eprintln!("  seed {seed} rotation {rotation}: {error}");
            }
        }
    }

    let census = census.borrow();
    println!(
        "{games} games ({errors} errors), {decisions} decisions, {:.1}s",
        started.elapsed().as_secs_f64()
    );
    println!(
        "{} unseen admitted names; those seen at least {min_count} times:",
        census.len()
    );
    let mut kept: Vec<(&String, &(usize, String))> = census
        .iter()
        .filter(|(_, (count, _))| *count >= min_count)
        .collect();
    kept.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    for (name, (count, subtype)) in &kept {
        println!("{count:>8}  {name:<60}  first under {subtype}");
    }
    if let Some(path) = argument("--names-out") {
        let text: String = kept.iter().map(|(name, _)| format!("{name}\n")).collect();
        std::fs::write(&path, text)
            .unwrap_or_else(|error| refuse(&format!("writing {path}: {error}")));
        println!("wrote {} names to {path}", kept.len());
    }
}
