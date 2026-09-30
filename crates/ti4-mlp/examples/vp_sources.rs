//! Read-only VP attribution: fixed opponents; final VP reconciled with scored cards and ledger.
use rayon::prelude::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_model::{
    content_types::{ContentType, DEFAULT},
    id::{FactionId, PlayerId},
};
const FACTIONS: [&str; 6] = ["sol", "letnev", "xxcha", "hacan", "jolnar", "l1z1x"];
fn arg(k: &str) -> Option<String> {
    let a: Vec<_> = std::env::args().collect();
    a.windows(2).find(|p| p[0] == k).map(|p| p[1].clone())
}
#[derive(Default)]
struct Log {
    hash: Sha256,
    choices: Vec<Value>,
    /// Every seat's decisions this game, by policy head: (count, seconds spent deciding).
    heads: BTreeMap<String, (usize, f64)>,
}
struct Watch {
    inner: Box<dyn Decider>,
    log: Rc<RefCell<Log>>,
    capture: bool,
}
impl Watch {
    fn time(&self, c: &Choice, started: std::time::Instant) {
        let mut l = self.log.borrow_mut();
        let entry = l
            .heads
            .entry(ti4_policy::learned::decision_head(c).to_owned())
            .or_default();
        entry.0 += 1;
        entry.1 += started.elapsed().as_secs_f64();
    }
    fn record(&self, c: &Choice, a: &ChoiceOption) {
        let mut l = self.log.borrow_mut();
        l.hash.update(format!("{c:?}{a:?}"));
        if self.capture {
            let subtype = c.context.as_ref().map(|x| x.subtype.as_str()).unwrap_or("");
            if subtype.contains("scor")
                || subtype.contains("secret")
                || ti4_policy::learned::decision_head(c) == "strategy"
            {
                l.choices.push(json!({"round":c.context.as_ref().map(|x|x.round),"subtype":subtype,"prompt":c.prompt,"options":c.options.iter().map(|o|o.id.clone()).collect::<Vec<_>>(),"chosen":a.id,"decline":a.is_decline()}));
            }
        }
    }
}
impl Decider for Watch {
    fn choose(&mut self, c: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let started = std::time::Instant::now();
        let a = self.inner.choose(c)?;
        self.time(c, started);
        self.record(c, &a);
        Ok(a)
    }
    fn choose_seeing(
        &mut self,
        c: &Choice,
        s: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let started = std::time::Instant::now();
        let a = self.inner.choose_seeing(c, s)?;
        self.time(c, started);
        self.record(c, &a);
        Ok(a)
    }
}
fn play(
    candidate: &Rc<ti4_mlp::Actor>,
    opponent: &Rc<ti4_mlp::Actor>,
    vocab: &ti4_policy::vocabulary::Vocabulary,
    pool: &Arc<ti4_sim::MapPool>,
    seed: u64,
    rotation: usize,
    seat: usize,
    temperature: f64,
    capture: bool,
) -> Result<Value, String> {
    let players: Vec<_> = (0..6).map(|i| PlayerId::new(format!("seat{i}"))).collect();
    let me = &players[seat];
    let assignments: BTreeMap<_, _> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.clone(),
                ti4_training::rollout::seated_faction(
                    &FACTIONS.map(FactionId::new),
                    seed,
                    rotation,
                    i,
                ),
            )
        })
        .collect();
    let log = Rc::new(RefCell::new(Log::default()));
    let mut statuses = Vec::new();
    let mut game = ti4_training::rollout::setup_game_with_decider_factory(
        ContentStore::embedded(),
        &players,
        &assignments,
        DEFAULT,
        seed,
        &ti4_training::rollout::OpeningMap::PythonPool {
            pool: Arc::clone(pool),
            tile_seed_offset: 0,
        },
        |baselines| {
            let mut bots: BTreeMap<PlayerId, Box<dyn Decider>> = BTreeMap::new();
            for (i, p) in players.iter().enumerate() {
                let actor = if i == seat { candidate } else { opponent };
                let (d, s) = ti4_mlp::bot::MlpBot::sharing(
                    actor,
                    vocab.clone(),
                    ti4_mlp::FactionRow::of(assignments[p].as_str()).unwrap(),
                    seed.wrapping_mul(1_000_003).wrapping_add(i as u64),
                )
                .at_temperature(temperature)
                .from_setup(baselines[p])
                .seat();
                statuses.push(s);
                bots.insert(
                    p.clone(),
                    Box::new(Watch {
                        inner: d,
                        log: Rc::clone(&log),
                        capture: capture && i == seat,
                    }),
                );
            }
            Ok(bots)
        },
    )?;
    // --diplomacy: structured diplomacy on, as the trainer enables it.
    if std::env::args().any(|a| a == "--diplomacy") {
        game.state.diplomacy =
            ti4_model::DiplomacyState::for_players(&game.state.seating_order, true);
    }
    let initial = game.state.player(me).unwrap().victory_points;
    // Technologies each seat starts with, so the table can report how many were researched.
    let starting_tech: BTreeMap<String, usize> = players
        .iter()
        .map(|p| {
            (
                assignments[p].to_string(),
                game.state.player(p).map_or(0, |x| x.technologies.len()),
            )
        })
        .collect();
    let game_started = std::time::Instant::now();
    let target = game.state.round + 4;
    let mut steps = 0;
    let mut awards = Vec::new();
    let mut ledger = Vec::new();
    let mut reveals: Vec<Value> = game
        .state
        .revealed_objectives
        .iter()
        .map(|id| json!({"id":id,"revealed_round":game.state.round,"available_round":game.state.round,"source":"initial"}))
        .collect();
    // Every Support for the Throne that changes hands at the table, with the deal that moved it:
    // the latest revision seen for each deal, and the deal whose terms were applied last.
    let mut supports = Vec::new();
    let mut revisions: BTreeMap<u64, (PlayerId, PlayerId, ti4_model::diplomacy::DealRevision)> =
        BTreeMap::new();
    let mut parties: BTreeMap<u64, (PlayerId, PlayerId)> = BTreeMap::new();
    let mut last_applied: Option<u64> = None;
    let faction = |p: &PlayerId| assignments[p].to_string();
    while !game.state.finished && game.state.round < target && steps < 400_000 {
        let round = game.state.round;
        let journal_len = game.state.diplomacy.journal.len();
        let holders_before = game.state.support_holders.clone();
        let phase = game.state.phase;
        let old_revealed = game.state.revealed_objectives.clone();
        let old = game
            .state
            .scored_objectives
            .get(me)
            .cloned()
            .unwrap_or_default();
        let n = game.state.vp_ledger.len();
        let result = game.step();
        if let Some(e) = result.error {
            return Err(format!("{seed}/{rotation}/{seat}: {e:?}"));
        }
        steps += 1;
        for entry in &game.state.diplomacy.journal[journal_len..] {
            use ti4_model::diplomacy::DiplomacyEvent as E;
            match &entry.event {
                E::Offered {
                    deal_id,
                    proposer,
                    recipient,
                    revision,
                    ..
                } => {
                    parties.insert(deal_id.0, (proposer.clone(), recipient.clone()));
                    revisions.insert(
                        deal_id.0,
                        (proposer.clone(), recipient.clone(), revision.clone()),
                    );
                }
                E::Countered { deal_id, revision } => {
                    if let Some((a, b)) = parties.get(&deal_id.0).cloned() {
                        revisions.insert(deal_id.0, (a, b, revision.clone()));
                    }
                }
                E::ImmediateApplied { deal_id, .. } => last_applied = Some(deal_id.0),
                _ => {}
            }
        }
        if game.state.support_holders != holders_before {
            for (owner, holder) in &game.state.support_holders {
                if holders_before.get(owner) != Some(holder) {
                    let deal = last_applied.and_then(|id| revisions.get(&id)).map(|(a, b, r)| {
                        let side = |terms: &[ti4_model::diplomacy::DealTerm]| {
                            terms
                                .iter()
                                .map(ti4_engine::diplomacy::builder::describe)
                                .collect::<Vec<_>>()
                        };
                        json!({"proposer":faction(a),"recipient":faction(b),"revision":r.number,
                            "proposer_gives":side(&r.proposer_terms),"recipient_gives":side(&r.recipient_terms)})
                    });
                    supports.push(json!({"round":round,"event":"given","owner":faction(owner),"holder":faction(holder),"deal":deal}));
                }
            }
            for (owner, holder) in &holders_before {
                if !game.state.support_holders.contains_key(owner) {
                    supports.push(json!({"round":round,"event":"returned","owner":faction(owner),"holder":faction(holder)}));
                }
            }
        }
        if capture {
            for id in game
                .state
                .revealed_objectives
                .iter()
                .filter(|id| !old_revealed.contains(id))
            {
                // Status reveals happen after that round's objective-scoring step, so the card's
                // first normal scoring opportunity is the following round. Other effects can
                // reveal during another phase and leave a same-round opportunity.
                let available_round = if phase == ti4_model::Phase::Status {
                    round + 1
                } else {
                    round
                };
                reveals.push(json!({"id":id,"revealed_round":round,"available_round":available_round,"source":format!("{phase:?}")}));
            }
            if let Some(scored) = game.state.scored_objectives.get(me) {
                for id in scored.difference(&old) {
                    let content = ContentStore::embedded();
                    let public = content
                        .get(ContentType::PublicObjectives, id.as_str())
                        .is_some();
                    let points = ti4_engine::objectives::points_for(content, id)
                        .ok_or_else(|| format!("unknown score {id}"))?;
                    awards.push(json!({"round":round,"id":id,"kind":if public{"public"}else{"secret"},"points":points}));
                }
            }
            for (p, d, r) in &game.state.vp_ledger[n..] {
                if p == me {
                    ledger.push(json!({"round":round,"delta":d,"reason":r}));
                }
            }
        }
    }
    if !game.state.finished && game.state.round < target {
        return Err("step cap".into());
    }
    for s in statuses {
        s.into_result().map_err(|e| e.to_string())?;
    }
    let l = log.borrow();
    Ok(
        json!({"seed":seed,"rotation":rotation,"seat":seat,"faction":assignments[me],"supports":supports,"seconds":game_started.elapsed().as_secs_f64(),"heads":l.heads,
            "tech_researched":players.iter().map(|p|(faction(p),game.state.player(p).map_or(0,|x|x.technologies.len()).saturating_sub(starting_tech[&faction(p)]))).collect::<BTreeMap<_,_>>(),"table_vp":players.iter().map(|p|(faction(p),game.state.player(p).map_or(0,|x|x.victory_points))).collect::<BTreeMap<_,_>>(),"temperature":temperature,"initial":initial,"vp":game.state.player(me).unwrap().victory_points,"secret_hand":game.state.player(me).unwrap().secret_objectives,"reveals":reveals,"awards":awards,"ledger":ledger,"choices":l.choices,"hashes":[format!("{:x}",l.hash.clone().finalize()),format!("{:x}",Sha256::digest(serde_json::to_vec(&game.events).unwrap())),format!("{:x}",Sha256::digest(serde_json::to_vec(&game.state).unwrap()))]}),
    )
}
fn main() {
    ti4_tensor::configure_deterministic(20_260_821).unwrap();
    let bundle = arg("--bundle").expect("bundle");
    let opponent =
        arg("--opponent").unwrap_or("out/checkpoints/stage2-mlp-shaped/checkpoint-473312".into());
    let c = ti4_mlp::bundle::read(std::path::Path::new(&bundle)).unwrap();
    let o = ti4_mlp::bundle::read(std::path::Path::new(
        if std::env::args().any(|a| a == "--self-play") {
            &bundle
        } else {
            &opponent
        },
    ))
    .unwrap();
    let pool = Arc::new(
        ti4_sim::MapPool::from_reader(std::io::Cursor::new(
            ti4_sim::artifacts::read_and_verify_pool_role(
                std::path::Path::new("out/pools/full_np8_12_holdout.json"),
                &[ti4_sim::artifacts::ArtifactRole::Validation],
            )
            .unwrap(),
        ))
        .unwrap(),
    );
    let seeds: u64 = arg("--seeds").unwrap_or("4".into()).parse().unwrap();
    let base: u64 = arg("--seed-base")
        .unwrap_or("910001000".into())
        .parse()
        .unwrap();
    let threads: usize = arg("--threads").unwrap_or("24".into()).parse().unwrap();
    let temperature: f64 = arg("--temperature")
        .unwrap_or("0.001".into())
        .parse()
        .ok()
        .filter(|value: &f64| value.is_finite() && *value > 0.0)
        .expect("--temperature must be a finite positive number");
    let verify = std::env::args().any(|a| a == "--verify");
    eprintln!(
        "vp_sources candidate={bundle} opponent={opponent} temperature={temperature} seeds={seeds} seed_base={base} threads={threads}"
    );
    // --self-play: the candidate is also every opponent, so each seed and rotation is one game.
    let seats = if std::env::args().any(|a| a == "--self-play") {
        1
    } else {
        6
    };
    let jobs: Vec<_> = (base..base + seeds)
        .flat_map(|s| (0..6).flat_map(move |r| (0..seats).map(move |p| (s, r, p))))
        .collect();
    let chunks: Vec<_> = jobs
        .chunks(jobs.len().div_ceil(threads))
        .map(|j| {
            (
                c.actor.inference_copy(),
                o.actor.inference_copy(),
                j.to_vec(),
            )
        })
        .collect();
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();
    let results: Vec<_> = chunks
        .into_par_iter()
        .flat_map_iter(|(ca, oa, j)| {
            let ca = Rc::new(ca);
            let oa = Rc::new(oa);
            j.into_iter()
                .map(|(s, r, p)| {
                    let a =
                        play(&ca, &oa, &c.vocabulary, &pool, s, r, p, temperature, true).unwrap();
                    if verify {
                        let b = play(&ca, &oa, &c.vocabulary, &pool, s, r, p, temperature, false)
                            .unwrap();
                        assert_eq!(a["hashes"], b["hashes"]);
                    }
                    a
                })
                .collect::<Vec<_>>()
        })
        .collect();
    for r in results {
        println!("{r}");
    }
}
