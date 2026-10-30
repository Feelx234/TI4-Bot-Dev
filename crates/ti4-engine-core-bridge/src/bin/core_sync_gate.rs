//! Gate 4 of docs/ENGINE_CORE_STATE.md (round trip) for the core_sync codecs, all slices.
//!
//! ```text
//! core_sync_gate CONFIG GAMES STRIDE [--strict]
//! ```
//! Positions: the opening of every map template (all lobby factions), `fixtures::game` tables of 3,
//! 4 and 6 seats, and every STRIDE-th position of the first GAMES games of CONFIG. For each:
//!
//! 1. `load` every slice (an `Unmapped` is counted by reason; it is a fallback to the string engine,
//!    not a failure, unless `--strict`);
//! 2. `store` every slice into a clone of the **previous position's** state (so a field no codec
//!    writes would keep the other position's value) and compare the serialised bytes with the
//!    position: `store(load(g)) == g`;
//! 3. `load` the stored state again with the same tables and compare the core states:
//!    `load(store(x)) == x`;
//! 4. time every slice's load and store, and count its string-side JSON bytes (lend cost table).
//!
//! Exit status 1 when any round trip differs.

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::Value;
use ti4_engine::core_sync::{Codec, Ctx, Slice, SliceSet, Sources};
use ti4_engine_core::state::CoreState;
use ti4_engine_core_bridge::workload::{Position, golden_positions, template_openings};
use ti4_model::state::GameState;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn slice_of_key(key: &str) -> Slice {
    match key {
        "players" => Slice::Players,
        "seating_order" => Slice::Seats,
        "speaker" | "phase" | "round" | "active" | "finished" | "active_system" | "strategy_cards_per_player"
        | "rules_version" | "thunders_edge" | "legacy_device_ids" | "custodians_removed" | "fracture_in_play"
        | "nexus_unlocked" | "styx_holder" | "thunders_edge_system" | "galactic_event" | "ion_storm"
        | "influence_pays_for_units" | "combat_round_seq" | "production_seq" | "activation_seq" | "agenda_seq"
        | "turn_seq" | "action_turn" | "feat_occurrence_seq" | "combat_draw_round" => Slice::Meta,
        "diplomacy" | "transactions_this_round" | "negotiations_this_action" | "negotiations_this_round"
        | "transactions_this_turn" | "traded_goods" | "traded_goods_for_promissory" | "promises"
        | "support_holders" => Slice::Trade,
        "unclaimed_strategy_cards" | "strategy_card_goods" | "card_initiative" | "initiative_overrides" => Slice::Strategy,
        "board" | "exhausted_planets" | "planet_attachments" | "placed_planets" | "purged_systems" => Slice::Board,
        "pending" => Slice::Pending,
        "revealed_objectives" | "scored_objectives" | "scored_feat_occurrences" => Slice::Objectives,
        "objective_deck" | "exploration_decks" | "relic_deck" | "agenda_deck" | "action_card_deck" | "secret_deck" => {
            Slice::Decks
        }
        "discarded_action_cards" | "salvage_yard" | "promissory_notes" | "promissory_faceup" => Slice::Cards,
        "exploration_log" | "frontier_tokens" | "ingress_tokens" | "breach_tokens" | "expedition_slices"
        | "wormhole_tokens" => Slice::Explore,
        "agenda_log" | "agenda_casts" | "laws" | "agenda_veto_replacement" | "covert_agenda"
        | "agenda_elected_override" | "agenda_votes" | "agenda_choices" | "agenda_in_play" | "agenda_predictions" => {
            Slice::Agenda
        }
        "relic_log" | "vp_ledger" => Slice::Ledger,
        "rng_seed" => Slice::Rng,
        "faction_marks" => Slice::Marks,
        _ => Slice::Transient,
    }
}

#[derive(Default, Clone, Copy)]
struct Cost {
    load_ns: u128,
    store_ns: u128,
    bytes: u128,
    n: u128,
}

fn json(state: &GameState) -> Value {
    serde_json::to_value(state).expect("state serialises")
}

fn differing_keys(a: &Value, b: &Value) -> Vec<String> {
    let (Value::Object(a), Value::Object(b)) = (a, b) else { return vec!["<not objects>".into()] };
    let mut keys: Vec<String> = a.keys().chain(b.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|k| a.get(k) != b.get(k)).collect()
}

fn fixtures(galaxy: &ti4_content::galaxy::Galaxy) -> Vec<Position> {
    let mut out = Vec::new();
    for seats in [&["a", "b", "c"][..], &["a", "b", "c", "d"], &["a", "b", "c", "d", "e", "f"]] {
        out.push(Position {
            label: format!("fixture {} seats", seats.len()),
            state: {
                // the placeholder faction `generic` is not in the corpus: seat real ones
                let mut state = ti4_engine::fixtures::game(seats);
                for (i, p) in state.players.iter_mut().enumerate() {
                    p.faction = ti4_model::id::FactionId::new(ti4_engine::generated::IdStr::as_str(
                        ti4_engine_core::generated::Faction::ALL[i],
                    ));
                }
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state
            },
            galaxy: galaxy.clone(),
            sources: ti4_model::content_types::POK,
        });
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    let config = args.first().map_or("tools/engine_profile/profile-20.json", String::as_str);
    let games: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let stride: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let (mut positions, refused) = template_openings();
    println!("template openings {} (refused {})", positions.len(), refused.len());
    let templates = positions.len();
    let galaxy = positions.first().expect("a template").galaxy.clone();
    positions.extend(fixtures(&galaxy));
    let fixture_count = positions.len() - templates;
    let (golden, hashes) = golden_positions(config, games, stride).expect("golden games");
    println!("golden games {} positions {}", hashes.len(), golden.len());
    positions.extend(golden);

    let mut unmapped: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut mismatches: Vec<String> = Vec::new();
    let mut roundtrip_core_diff: Vec<String> = Vec::new();
    let mut costs = [Cost::default(); Slice::COUNT];
    let (mut loaded, mut checked_core) = (0usize, 0usize);
    let (mut texts_max, mut blobs_max) = (0usize, 0usize);
    let mut previous: Option<GameState> = None;

    for p in &positions {
        let mut cx = match Ctx::of(&p.state) {
            Ok(cx) => cx,
            Err(e) => {
                let entry = unmapped.entry(format!("{e}")).or_default();
                entry.0 += 1;
                entry.1.clone_from(&p.label);
                continue;
            }
        };
        let src = Sources { galaxy: Some(&p.galaxy), rng: None };
        let mut core = CoreState::with_loaded(SliceSet::EMPTY);
        cx.attach(&mut core);
        let mut ok = true;
        let value = json(&p.state);
        for slice in Slice::ALL {
            let t = Instant::now();
            let r = Codec::load_slice(&mut cx, &p.state, src, slice, &mut core);
            let ns = t.elapsed().as_nanos();
            match r {
                Ok(()) => {
                    let c = &mut costs[slice as usize];
                    c.load_ns += ns;
                    c.n += 1;
                }
                Err(e) => {
                    ok = false;
                    let entry = unmapped.entry(format!("{:?}: {} ({})", e.slice, e.what, e.text.chars().take(24).collect::<String>())).or_default();
                    entry.0 += 1;
                    entry.1.clone_from(&p.label);
                    break;
                }
            }
        }
        if let Value::Object(map) = &value {
            for (k, v) in map {
                costs[slice_of_key(k) as usize].bytes += serde_json::to_vec(v).map_or(0, |b| b.len() as u128);
            }
        }
        if !ok {
            previous = Some(p.state.clone());
            continue;
        }
        loaded += 1;
        cx.adopt(&core);
        texts_max = texts_max.max(core.texts.codes.len());
        blobs_max = blobs_max.max(core.texts.blobs.len());
        let mut back = previous.clone().unwrap_or_else(|| p.state.clone());
        for slice in Slice::ALL {
            let t = Instant::now();
            let r = Codec::store_slice(&cx, &core, &mut back, slice);
            costs[slice as usize].store_ns += t.elapsed().as_nanos();
            if let Err(e) = r {
                mismatches.push(format!("{}: store {e}", p.label));
            }
        }
        let back_value = json(&back);
        if back_value != value {
            let keys = differing_keys(&value, &back_value);
            mismatches.push(format!("{}: bytes differ in {}", p.label, keys.join(", ")));
        } else if serde_json::to_vec(&back).ok() != serde_json::to_vec(&p.state).ok() {
            mismatches.push(format!("{}: serialised bytes differ (same JSON value)", p.label));
        }
        // load(store(x)) == x, with the same tables (the stored state has the same owners).
        match Codec::load(&mut cx, &back, src, SliceSet::ALL) {
            Ok(again) => {
                checked_core += 1;
                if again != core {
                    roundtrip_core_diff.push(p.label.clone());
                }
            }
            Err(e) => roundtrip_core_diff.push(format!("{}: reload {e}", p.label)),
        }
        previous = Some(p.state.clone());
    }

    println!(
        "positions {} (templates {templates}, fixtures {fixture_count}, golden {}) | fully loaded {loaded} | unmapped positions {} | store(load) mismatches {} | load(store) core diffs {} of {checked_core}",
        positions.len(),
        positions.len() - templates - fixture_count,
        positions.len() - loaded,
        mismatches.len(),
        roundtrip_core_diff.len()
    );
    for (reason, (n, example)) in &unmapped {
        println!("UNMAPPED x{n}: {reason} (e.g. {example})");
    }
    for m in mismatches.iter().take(40) {
        println!("MISMATCH {m}");
    }
    for m in roundtrip_core_diff.iter().take(20) {
        println!("CORE DIFF {m}");
    }
    println!("text tables high-water over {loaded} loaded positions: codes {texts_max}, blobs {blobs_max}");
    println!("size_of CoreState {} B, Player {} B", std::mem::size_of::<CoreState>(), std::mem::size_of::<ti4_engine_core::state::Player>());
    println!("\nlend cost per slice (mean over {loaded} loaded positions): string JSON bytes, typed inline bytes, load ns, store ns");
    let inline = |s: Slice| -> usize {
        use ti4_engine_core::state as st;
        use std::mem::size_of;
        match s {
            Slice::Meta => size_of::<st::Meta>(),
            Slice::Seats => size_of::<st::Seats>(),
            Slice::Players => size_of::<st::Players>(),
            Slice::Board => size_of::<st::Board>(),
            Slice::Map => size_of::<st::MapSlice>(),
            Slice::Decks => size_of::<st::Decks>(),
            Slice::Cards => size_of::<st::CardZones>(),
            Slice::Objectives => size_of::<st::Objectives>(),
            Slice::Agenda => size_of::<st::AgendaState>(),
            Slice::Strategy => size_of::<st::StrategyState>(),
            Slice::Explore => size_of::<st::ExploreState>(),
            Slice::Trade => size_of::<st::TradeState>(),
            Slice::Marks => size_of::<st::Marks>(),
            Slice::Transient => size_of::<st::Transient>(),
            Slice::Pending => size_of::<st::Pending>(),
            Slice::Rng => size_of::<st::Rng>(),
            Slice::Ledger => size_of::<st::Ledger>(),
        }
    };
    for s in Slice::ALL {
        let c = costs[s as usize];
        let n = c.n.max(1);
        println!(
            "| {s:?} | {} | {} | {} | {} |",
            c.bytes / (positions.len() as u128).max(1),
            inline(s),
            c.load_ns / n,
            c.store_ns / n
        );
    }
    if !mismatches.is_empty() || !roundtrip_core_diff.is_empty() || (strict && loaded != positions.len()) {
        std::process::exit(1);
    }
}
