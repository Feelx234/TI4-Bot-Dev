//! The string timing [`Resolver`] as a portable image and back (docs/todo/D2-prereqs.md D2-4a).
//!
//! After the flip the state is the long-lived owner and a string window (a lent body, D2-5) runs on a
//! resolver built for the occasion. This module is that contract, in code: what a resolver is made of,
//! where each part comes from when it is rebuilt, and what a window hands back. `TI4_CHURN=resolver`
//! (`crate::churn`) rebuilds the live resolver of a running game through it before every step, which
//! proves the table below is complete (every bench decision hash must stay equal).
//!
//! | `Resolver` field | after a rebuild it comes from | in the image | back out (delta) |
//! |---|---|---|---|
//! | `registry` (closures) | derived: `reactions::arm` on the state | no (`registry_ids` is compared in tests) | nothing |
//! | `initiative_order` | **its own value from `Game::new`** (the state's `initiative_order()` changes after strategy cards are chosen; `player_order` reads this one in every non-speaker phase) | `initiative` | nothing (never changes) |
//! | `seating_order`, `active_player`, `speaker`, `phase`, `round_number`, `turn_number` | the image ([`Context::Image`], exact) or the state exactly as `Game::sync_timing_context` ([`Context::State`]) | `seating`, `active`, `speaker`, `phase`, `round`, `turn` | nothing |
//! | `used` (ordered `(ability, Trigger/Turn/Round, n)`) | spent again in order | `used` | the changed suffix |
//! | `emission_stack`, `maximum_depth` | the image | `emission_stack`, `max_depth` | nothing (restored) |
//! | `applied_events`, `log` | the image, whole (string passthrough: the typed event model is P1's, no second one here) | `applied`, `log` | the changed suffix |
//! | `table` (deciders, decision log) | **moved** out of the old resolver and back, never rebuilt | no | n/a |
//! | `relation_being_resolved`, `participation` | defaults (`None`): an image is only taken where no window is being resolved ([`export`] refuses otherwise) | no | n/a |
//! | event id counter (`EventSequence`) | the image | `next_event_id` | read back |
//!
//! Text <-> typed terms uses the shim renderers of `kernel_sync::text` through a [`Render`] (seats are
//! the sorted player ids, ability ids and event names are interned in it): an image is meaningful only
//! together with the `Render` it was exported with.

use ti4_engine_core::choice::reaction::AbilityId;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::resolver::UsedScope;
use ti4_engine_core::containers::{Cow, SmallList};
use ti4_engine_core::state::{EventKind, ResolverState, Slice};
use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, Phase};

use crate::choice::Table;
use crate::core_sync::{R, Unmapped};
use crate::event::{Event, EventSequence};
use crate::kernel_sync::{ability_id, ability_text, event_kind, event_text};
use crate::render::Render;
use crate::timing::{FrequencyScope, Resolver};

const S: Slice = Slice::Pending;

fn bad(what: &'static str, text: impl ToString) -> Unmapped {
    crate::core_sync::bad(S, what, text)
}

/// Everything of a [`Resolver`] that outlives a step, in core terms (see the module table).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolverImage {
    pub initiative: Vec<Seat>,
    pub seating: Vec<Seat>,
    pub active: Option<Seat>,
    pub speaker: Option<Seat>,
    pub phase: Phase,
    pub round: u64,
    pub turn: u64,
    pub max_depth: usize,
    /// In the order the keys were spent.
    pub used: Vec<(AbilityId, UsedScope, u64)>,
    /// Open emissions, outermost first.
    pub emission_stack: Vec<(EventKind, u64)>,
    pub applied: Vec<Event>,
    pub log: Vec<String>,
    pub next_event_id: u64,
}

/// Where the clock and the seats of a rebuilt resolver come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// The image's own values: the rebuilt resolver is the exported one.
    Image,
    /// The state, as `Game::sync_timing_context` writes them before an emission.
    State,
}

/// What a window changed: per list, how many leading entries are unchanged and what replaces the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolverDelta {
    pub used_keep: usize,
    pub used: Vec<(AbilityId, UsedScope, u64)>,
    pub applied_keep: usize,
    pub applied: Vec<Event>,
    pub log_keep: usize,
    pub log: Vec<String>,
    pub next_event_id: u64,
}

/// The `Render` of a game: seats are the player ids in text order (the owner table order).
#[must_use]
pub fn render_for(state: &GameState) -> Render {
    let mut ids: Vec<String> = state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
    ids.sort();
    ids.dedup();
    Render::new(ids)
}

fn seat_of(r: &Render, p: &PlayerId) -> R<Seat> {
    r.seat(p.as_str()).ok_or_else(|| bad("resolver player not in the table", p))
}

fn seats_of(r: &Render, ps: &[PlayerId]) -> R<Vec<Seat>> {
    ps.iter().map(|p| seat_of(r, p)).collect()
}

fn scope_of(scope: &FrequencyScope) -> (UsedScope, u64) {
    match scope {
        FrequencyScope::Trigger(n) => (UsedScope::Trigger, *n),
        FrequencyScope::Turn(n) => (UsedScope::Turn, *n),
        FrequencyScope::Round(n) => (UsedScope::Round, *n),
    }
}

const fn scope_back(scope: UsedScope, n: u64) -> FrequencyScope {
    match scope {
        UsedScope::Trigger => FrequencyScope::Trigger(n),
        UsedScope::Turn => FrequencyScope::Turn(n),
        UsedScope::Round => FrequencyScope::Round(n),
    }
}

/// The image of `res` and the event counter `seq`.
///
/// # Errors
/// A player the `Render` does not know, or a window being resolved (an image is taken between windows).
pub fn export(res: &Resolver, seq: &EventSequence, r: &Render) -> R<ResolverImage> {
    if res.relation_being_resolved().is_some() {
        return Err(bad("resolver window open", ""));
    }
    let (round, turn) = res.counters();
    Ok(ResolverImage {
        initiative: seats_of(r, res.initiative_order())?,
        seating: seats_of(r, res.seating_order())?,
        active: res.active_player().map(|p| seat_of(r, p)).transpose()?,
        speaker: res.speaker().map(|p| seat_of(r, p)).transpose()?,
        phase: res.phase(),
        round,
        turn,
        max_depth: res.maximum_depth(),
        used: res
            .used_keys()
            .iter()
            .map(|(id, scope)| {
                let (scope, n) = scope_of(scope);
                (ability_id(r, id), scope, n)
            })
            .collect(),
        emission_stack: res.emission_stack().iter().map(|(kind, id)| (event_kind(r, kind), *id)).collect(),
        applied: res.applied_events().to_vec(),
        log: res.log().to_vec(),
        next_event_id: seq.peek(),
    })
}

/// A resolver made from `img`: the standing registry armed from `state` (the one thing that is derived), the
/// clock per `context`, the keys spent again in order, the journal and trace restored, `table` moved in.
/// `seq` continues the image's event ids.
#[must_use]
pub fn rebuild(state: &GameState, img: &ResolverImage, table: Table, r: &Render, context: Context, seq: &mut EventSequence) -> Resolver {
    let player = |s: Seat| r.player(s);
    let mut res = Resolver::new(
        img.initiative.iter().map(|s| player(*s)).collect(),
        img.active.map(player),
        table,
    );
    crate::reactions::arm(&mut res, state);
    match context {
        Context::Image => {
            res.set_phase(img.phase);
            res.set_counters(img.round, img.turn);
            res.set_seating_order(img.seating.iter().map(|s| player(*s)).collect());
            res.set_active_player(img.active.map(player));
            res.set_speaker(img.speaker.map(player));
        }
        Context::State => {
            res.set_phase(state.phase);
            res.sync_lifecycle(state.round, state.turn_seq);
            res.set_seating_order(state.seating_order.clone());
            res.set_active_player(state.active.clone());
            res.set_speaker(Some(state.speaker.clone()));
        }
    }
    res.set_maximum_depth(img.max_depth);
    for (id, scope, n) in &img.used {
        res.spend_key(ability_text(r, *id), scope_back(*scope, *n));
    }
    res.set_emission_stack(img.emission_stack.iter().map(|(kind, id)| (event_text(r, *kind), *id)).collect());
    res.set_journal(img.log.clone(), img.applied.clone());
    *seq = EventSequence::from_next(img.next_event_id);
    res
}

/// The persistent part of the image as the core state carries it (`CoreState::pending.resolver`, D2-4b): the
/// spent keys, the initiative order, the open emissions and the next trace id. The clock, the seats, the
/// journal and the trace are not in the state: they are the state's own or the events view's.
///
/// # Errors
/// More than eight seats in the initiative order.
pub fn to_state(img: &ResolverImage) -> R<ResolverState> {
    if img.initiative.len() > 8 {
        return Err(bad("initiative order of more than 8 seats", img.initiative.len()));
    }
    Ok(ResolverState {
        used: Cow::new(img.used.clone()),
        initiative: SmallList::from_slice(&img.initiative),
        emission_stack: img.emission_stack.clone(),
        next_id: img.next_event_id,
    })
}

/// `rest` with the persistent part replaced by the state's (the string side of loading a state: the clock and
/// the journal stay the caller's).
#[must_use]
pub fn with_state(rest: &ResolverImage, state: &ResolverState) -> ResolverImage {
    ResolverImage {
        used: state.used.to_vec(),
        initiative: state.initiative.as_slice().to_vec(),
        emission_stack: state.emission_stack.clone(),
        next_event_id: state.next_id,
        ..rest.clone()
    }
}

/// The persistent part of the string resolver a window found (`Recording::cfg`) or left (`Recording::after`), as the
/// core state carries it. `None` for a seat the `Render` does not know, more than eight seats of initiative, or a
/// scope text that is not `Trigger(n)` / `Turn(n)` / `Round(n)`.
#[must_use]
pub fn cfg_state(r: &Render, cfg: &crate::dual_window::ResolverCfg, next_id: u64) -> Option<ResolverState> {
    if cfg.initiative.len() > 8 {
        return None;
    }
    let initiative: Option<Vec<Seat>> = cfg.initiative.iter().map(|s| r.seat(s)).collect();
    let mut used = Vec::with_capacity(cfg.used.len());
    for (id, scope) in &cfg.used {
        let (kind, rest) = scope.split_once('(')?;
        let n: u64 = rest.trim_end_matches(')').parse().ok()?;
        let kind = match kind {
            "Trigger" => UsedScope::Trigger,
            "Turn" => UsedScope::Turn,
            "Round" => UsedScope::Round,
            _ => return None,
        };
        used.push((ability_id(r, id), kind, n));
    }
    Some(ResolverState {
        used: Cow::new(used),
        initiative: SmallList::from_slice(&initiative?),
        emission_stack: cfg.stack.iter().map(|(kind, id)| (event_kind(r, kind), *id)).collect(),
        next_id,
    })
}

/// A resolver for a string window that runs on a materialised state (`reverse_lend::with_string`): the standing registry
/// armed from `state`, the clock and the seats exactly as `Game::sync_timing_context` writes them, the persistent part
/// from `carried` (the state's `pending.resolver`), an empty trace and journal (what the window adds is read back with
/// [`export`] and [`to_state`]), `table` moved in. A `carried` without an initiative order (a state nobody stored a
/// resolver in) takes the state's own order of that moment. The counter continues at `carried.next_id`.
#[must_use]
pub fn rebuild_for_lend(state: &GameState, carried: &ResolverState, table: Table, r: &Render) -> (Resolver, EventSequence) {
    let initiative = if carried.initiative.is_empty() {
        state.initiative_order().iter().filter_map(|p| r.seat(p.as_str())).collect()
    } else {
        carried.initiative.as_slice().to_vec()
    };
    let img = ResolverImage {
        initiative,
        seating: Vec::new(),
        active: None,
        speaker: None,
        phase: state.phase,
        round: u64::from(state.round),
        turn: u64::from(state.turn_seq),
        max_depth: Resolver::DEFAULT_MAXIMUM_DEPTH,
        used: carried.used.to_vec(),
        emission_stack: carried.emission_stack.clone(),
        applied: Vec::new(),
        log: Vec::new(),
        next_event_id: carried.next_id,
    };
    let mut seq = EventSequence::new();
    let res = rebuild(state, &img, table, r, Context::State, &mut seq);
    (res, seq)
}

fn common_prefix<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// What `after` changed against `before`.
#[must_use]
pub fn delta(before: &ResolverImage, after: &ResolverImage) -> ResolverDelta {
    let used_keep = common_prefix(&before.used, &after.used);
    let applied_keep = common_prefix(&before.applied, &after.applied);
    let log_keep = common_prefix(&before.log, &after.log);
    ResolverDelta {
        used_keep,
        used: after.used[used_keep..].to_vec(),
        applied_keep,
        applied: after.applied[applied_keep..].to_vec(),
        log_keep,
        log: after.log[log_keep..].to_vec(),
        next_event_id: after.next_event_id,
    }
}

/// Apply what a window changed to the image it started from.
pub fn apply(img: &mut ResolverImage, d: &ResolverDelta) {
    img.used.truncate(d.used_keep);
    img.used.extend(d.used.iter().copied());
    img.applied.truncate(d.applied_keep);
    img.applied.extend(d.applied.iter().cloned());
    img.log.truncate(d.log_keep);
    img.log.extend(d.log.iter().cloned());
    img.next_event_id = d.next_event_id;
}

/// The churn step (`TI4_CHURN=resolver`): the live resolver of a game replaced by one rebuilt from its image.
///
/// # Panics
/// When the resolver cannot be exported: the churn mode is a test mode and says so loudly.
pub fn churn(state: &GameState, timing: &mut Resolver, seq: &mut EventSequence) {
    let r = render_for(state);
    let img = export(timing, seq, &r).unwrap_or_else(|e| panic!("TI4_CHURN=resolver: {e}"));
    let table = std::mem::take(timing.table_mut());
    *timing = rebuild(state, &img, table, &r, Context::Image, seq);
}

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;

    use super::*;
    use crate::game::Game;

    fn players(n: usize) -> Vec<PlayerId> {
        ["a", "b", "c", "d", "e", "f"][..n].iter().map(|p| PlayerId::new(*p)).collect()
    }

    fn game(n: usize, seed: u64) -> Game<'static> {
        let content = ContentStore::embedded();
        let state = crate::setup::start_game_seeded(content, &players(n), POK, None, seed).expect("setup");
        Game::with_seeded_random(state, content, seed)
    }

    /// Play `steps` steps; `f` sees the game before every `stride`-th step.
    fn play(g: &mut Game<'_>, steps: usize, stride: usize, mut f: impl FnMut(&Game<'_>)) {
        for i in 0..steps {
            if g.state.finished {
                break;
            }
            if i % stride == 0 {
                f(g);
            }
            assert!(g.step().error.is_none(), "step {i} refused");
        }
    }

    const GAMES: [(usize, u64); 4] = [(3, 7), (4, 11), (6, 23), (2, 5)];

    /// I1: export -> rebuild -> export is the identity, in both context modes where they agree, and the
    /// table is moved, not rebuilt.
    #[test]
    fn i1_the_image_round_trips_on_every_position_of_played_games() {
        let mut checked = 0;
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 5, |g| {
                let r = render_for(&g.state);
                let mut seq = g.event_sequence().clone();
                let img = export(&g.timing, &seq, &r).unwrap();
                let rebuilt = rebuild(&g.state, &img, Table::default(), &r, Context::Image, &mut seq);
                assert_eq!(export(&rebuilt, &seq, &r).unwrap(), img);
                assert_eq!(seq.peek(), img.next_event_id);
                checked += 1;
            });
        }
        assert!(checked > 200, "positions checked: {checked}");
    }

    /// A resolver that really spent keys of every scope (random games spend none: the standing slots are
    /// `Unlimited`), and the counter that went with it.
    fn spent_resolver() -> (Resolver, EventSequence) {
        use std::sync::Arc;

        use crate::timing::{Ability, Frequency, Relation};
        let a = PlayerId::new("a");
        let mut res = Resolver::new(players(2), Some(a.clone()), Table::default());
        res.sync_lifecycle(2, 5);
        let ability = |id: &str, f: Frequency| {
            Ability::new(id, a.clone(), "E", Relation::When, Arc::new(|_, _| Ok(()))).with_frequency(f)
        };
        res.register([
            ability("ability:t:trigger", Frequency::OncePerTrigger),
            ability("ability:t:turn", Frequency::OncePerTurn),
            ability("ability:t:round", Frequency::OncePerRound),
            ability("ability:t:free", Frequency::Unlimited),
        ]);
        let mut seq = EventSequence::new();
        for _ in 0..3 {
            let event = seq.next("E", std::collections::BTreeMap::new()).unwrap();
            res.emit(event, |_| {}).unwrap();
        }
        (res, seq)
    }

    /// I1 with keys spent: every scope round-trips, in the order spent, and nothing else changes.
    #[test]
    fn i1_spent_keys_of_every_scope_round_trip_in_order() {
        let (res, seq) = spent_resolver();
        let state = crate::setup::start_game_seeded(ContentStore::embedded(), &players(2), POK, None, 1).unwrap();
        let r = render_for(&state);
        let img = export(&res, &seq, &r).unwrap();
        let scopes: Vec<UsedScope> = img.used.iter().map(|(_, s, _)| *s).collect();
        for scope in [UsedScope::Trigger, UsedScope::Turn, UsedScope::Round] {
            assert!(scopes.contains(&scope), "{scope:?} was not spent: {:?}", img.used);
        }
        assert!(img.used.iter().any(|(_, s, n)| *s == UsedScope::Turn && *n == 5));
        assert!(img.used.iter().any(|(_, s, n)| *s == UsedScope::Round && *n == 2));
        // the Trigger keys name the three events
        let triggers: Vec<u64> = img.used.iter().filter(|(_, s, _)| *s == UsedScope::Trigger).map(|(_, _, n)| *n).collect();
        assert_eq!(triggers, vec![1, 2, 3]);
        let mut next = EventSequence::new();
        let rebuilt = rebuild(&state, &img, Table::default(), &r, Context::Image, &mut next);
        assert_eq!(export(&rebuilt, &next, &r).unwrap(), img);
        assert_eq!(rebuilt.used_keys(), res.used_keys(), "the same keys in the same order");
        assert_eq!(next.peek(), 4);
    }

    /// I2: arming the registry again from the current state gives the registry of the long-lived resolver
    /// (it would not if `arm` depended on mid-game state).
    #[test]
    fn i2_rearming_from_the_current_state_reproduces_the_registry() {
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 3, |g| {
                let r = render_for(&g.state);
                let mut seq = g.event_sequence().clone();
                let img = export(&g.timing, &seq, &r).unwrap();
                for context in [Context::Image, Context::State] {
                    let rebuilt = rebuild(&g.state, &img, Table::default(), &r, context, &mut seq);
                    assert_eq!(rebuilt.registry_ids(), g.timing.registry_ids(), "registry, {context:?}");
                }
            });
        }
    }

    /// I3: the priority order of a rebuilt resolver is the live one's in every phase. The image carries the
    /// resolver's own initiative order: the state's changes after the strategy cards are chosen (T4).
    #[test]
    fn i3_priority_order_survives_a_rebuild_and_the_state_initiative_would_not() {
        let mut state_initiative_differs = 0;
        let mut positions = 0;
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 2, |g| {
                let r = render_for(&g.state);
                let mut seq = g.event_sequence().clone();
                let img = export(&g.timing, &seq, &r).unwrap();
                let rebuilt = rebuild(&g.state, &img, Table::default(), &r, Context::Image, &mut seq);
                assert_eq!(rebuilt.player_order(), g.timing.player_order(), "phase {:?}", g.state.phase);
                assert_eq!(rebuilt.initiative_order(), g.timing.initiative_order());
                if g.state.initiative_order() != g.timing.initiative_order() {
                    state_initiative_differs += 1;
                }
                positions += 1;
            });
        }
        assert!(positions > 400);
        assert!(
            state_initiative_differs > 0,
            "the state's initiative order never differed from the resolver's: T4 would not be a trap, drop the field from the image"
        );
    }

    /// I4: the Trigger keys are event ids of the carried counter; a fresh counter would collide with them.
    #[test]
    fn i4_trigger_keys_stay_below_the_carried_event_counter() {
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 5, |g| {
                let r = render_for(&g.state);
                let img = export(&g.timing, g.event_sequence(), &r).unwrap();
                for (_, scope, id) in &img.used {
                    if *scope == UsedScope::Trigger {
                        assert!(*id < img.next_event_id, "a spent Trigger({id}) at counter {}", img.next_event_id);
                    }
                }
            });
        }
        // The counter is carried, not restarted: a Trigger key of a later event than a fresh counter's first ids.
        let g = game(3, 7);
        let r = render_for(&g.state);
        let mut seq = EventSequence::from_next(500);
        let mut img = export(&g.timing, &seq, &r).unwrap();
        img.used.push((ability_id(&r, "ability:test:once"), UsedScope::Trigger, 321));
        let mut fresh = EventSequence::new();
        let rebuilt = rebuild(&g.state, &img, Table::default(), &r, Context::Image, &mut fresh);
        assert_eq!(fresh.peek(), 500, "the rebuilt game continues the old counter");
        assert_eq!(export(&rebuilt, &fresh, &r).unwrap(), img);
        seq = EventSequence::new();
        assert!(seq.peek() < 321, "a fresh counter would hand out an id a spent Trigger key already names");
    }

    /// I5: a checkpoint of a rebuilt resolver undoes exactly the keys spent after the rebuild.
    #[test]
    fn i5_a_checkpoint_of_a_rebuilt_resolver_undoes_only_later_keys() {
        let mut g = game(3, 7);
        play(&mut g, 300, 1000, |_| {});
        let r = render_for(&g.state);
        let mut seq = g.event_sequence().clone();
        let img = export(&g.timing, &seq, &r).unwrap();
        let mut rebuilt = rebuild(&g.state, &img, Table::default(), &r, Context::Image, &mut seq);
        let before = rebuilt.used_keys().to_vec();
        let mark = rebuilt.checkpoint();
        rebuilt.spend_key("ability:test:later".into(), FrequencyScope::Round(99));
        rebuilt.spend_key("ability:test:later".into(), FrequencyScope::Trigger(1_000_000));
        assert_eq!(rebuilt.used_keys().len(), before.len() + 2);
        rebuilt.restore(mark);
        assert_eq!(rebuilt.used_keys(), before.as_slice());
    }

    /// The churn proof in miniature: a game whose resolver is rebuilt before every step plays exactly the
    /// game that is not (same final state, same decision log, same trace).
    #[test]
    fn rebuilding_before_every_step_changes_nothing() {
        for (n, seed) in GAMES {
            let mut plain = game(n, seed);
            let mut churned = game(n, seed);
            for i in 0..600 {
                if plain.state.finished {
                    break;
                }
                churned.churn_resolver();
                let (a, b) = (plain.step(), churned.step());
                assert_eq!(a.error, b.error, "step {i}");
                assert_eq!(plain.state, churned.state, "state after step {i} of game {seed}");
            }
            assert_eq!(plain.table.log, churned.table.log, "decision log of game {seed}");
            assert_eq!(plain.timing.log(), churned.timing.log(), "trace of game {seed}");
            assert_eq!(plain.timing.applied_events(), churned.timing.applied_events());
            assert_eq!(plain.events, churned.events);
        }
    }

    /// D2-4b, string side: the persistent part of the image is the state's `ResolverState`, and the core
    /// resolver made from it spends the very keys the string resolver spent, in the same order.
    #[test]
    fn the_image_is_the_core_state_and_the_core_resolver_made_from_it_agrees() {
        let (res, seq) = spent_resolver();
        let state = crate::setup::start_game_seeded(ContentStore::embedded(), &players(2), POK, None, 1).unwrap();
        let r = render_for(&state);
        let img = export(&res, &seq, &r).unwrap();
        let carried = to_state(&img).unwrap();
        assert_eq!(carried.next_id, 4);
        assert_eq!(carried.used.len(), img.used.len());
        let core = ti4_engine_core::kernel::Resolver::from_state(&carried, Cow::new(ti4_engine_core::kernel::Registry::default()));
        assert_eq!(core.used_keys(), img.used, "the core resolver spent the string resolver's keys in order");
        assert_eq!(core.into_state(carried.next_id), carried, "and hands them back unchanged");
        assert_eq!(with_state(&img, &carried), img, "the state plus the rest is the image");
        // a played game: the same holds at every sampled position
        let mut g = game(4, 11);
        play(&mut g, 400, 7, |g| {
            let r = render_for(&g.state);
            let img = export(&g.timing, g.event_sequence(), &r).unwrap();
            let carried = to_state(&img).unwrap();
            assert_eq!(with_state(&img, &carried), img);
        });
    }

    /// The churn step really replaces the resolver (a no-op would pass the equality test above): what is not in
    /// the image does not survive it, what is, does.
    #[test]
    fn the_churn_step_replaces_the_resolver_and_keeps_the_image() {
        use std::sync::Arc;

        use crate::timing::{Ability, Relation};
        let mut g = game(3, 7);
        play(&mut g, 120, 1000, |_| {});
        g.timing.spend_key("ability:test:spent".into(), FrequencyScope::Round(9));
        let ids = g.timing.registry_ids();
        g.timing.register([Ability::new("ability:test:extra", g.state.players[0].id.clone(), "E", Relation::When, Arc::new(|_, _| Ok(())))]);
        assert_eq!(g.timing.registry_ids().len(), ids.len() + 1);
        let r = render_for(&g.state);
        let before = export(&g.timing, g.event_sequence(), &r).unwrap();
        g.churn_resolver();
        assert_eq!(g.timing.registry_ids(), ids, "the registry is armed again from the state: the extra ability is gone");
        assert_eq!(export(&g.timing, g.event_sequence(), &r).unwrap(), before, "the image is what survives");
    }

    #[test]
    fn the_state_holds_eight_seats_of_initiative_and_no_more() {
        let (res, seq) = spent_resolver();
        let state = crate::setup::start_game_seeded(ContentStore::embedded(), &players(2), POK, None, 1).unwrap();
        let mut img = export(&res, &seq, &render_for(&state)).unwrap();
        img.initiative = (0..8).map(Seat).collect();
        assert_eq!(to_state(&img).unwrap().initiative.len(), 8);
        img.initiative = (0..9).map(Seat).collect();
        assert!(to_state(&img).is_err());
    }

    /// The two views of one string resolver (the configuration a window records and the image) carry the same persistent part.
    #[test]
    fn the_recorded_configuration_and_the_image_carry_the_same_state() {
        let (res, seq) = spent_resolver();
        let state = crate::setup::start_game_seeded(ContentStore::embedded(), &players(2), POK, None, 1).unwrap();
        let r = render_for(&state);
        let from_cfg = cfg_state(&r, &res.core_cfg(&POK), seq.peek()).expect("every seat is in the table");
        let from_image = to_state(&export(&res, &seq, &r).unwrap()).unwrap();
        assert_eq!(from_cfg, from_image);
        assert!(!from_cfg.used.is_empty());
        let mut stranger = res.core_cfg(&POK);
        stranger.initiative.push("nobody".into());
        assert!(cfg_state(&r, &stranger, 1).is_none(), "a seat the table does not know");
        let mut bad = res.core_cfg(&POK);
        bad.used.push(("ability:x".into(), "Weekly(2)".into()));
        assert!(cfg_state(&r, &bad, 1).is_none(), "a scope that is none of the three");
    }

    /// `TI4_CHURN=state` in miniature: a game whose state and streams go through the core before every step plays the game
    /// that does not (the churn step is loud on anything the codecs cannot hand back unchanged).
    #[test]
    fn sending_the_state_through_the_core_before_every_step_changes_nothing() {
        use crate::generated::IdStr;
        // Seated with real factions: a player with the placeholder faction `generic` is refused by the codecs (T1).
        let seated = |n: usize, seed: u64| {
            let names = ["a", "b", "c", "d", "e", "f"];
            let seats: Vec<(&str, &str)> = (0..n).map(|i| (names[i], ti4_engine_core::generated::Faction::ALL[i].as_str())).collect();
            let mut state = crate::fixtures::seated_game(&seats, POK);
            crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
            Game::with_seeded_random(state, ContentStore::embedded(), seed)
        };
        for (n, seed) in GAMES {
            let mut plain = seated(n, seed);
            let mut churned = seated(n, seed);
            for i in 0..600 {
                if plain.state.finished {
                    break;
                }
                churned.churn_state();
                // (printed only when the test fails: which game and step a dual difference belongs to)
                eprintln!("game {n}p seed {seed}: plain step {i}");
                let a = plain.step();
                eprintln!("game {n}p seed {seed}: churned step {i}");
                let b = churned.step();
                assert_eq!(a.error, b.error, "step {i}");
                assert_eq!(plain.state, churned.state, "state after step {i} of game {seed}");
            }
            assert_eq!(plain.table.log, churned.table.log, "decision log of game {seed}");
        }
    }

    #[test]
    fn a_configuration_with_more_than_eight_seats_of_initiative_is_not_a_state() {
        let state = crate::setup::start_game_seeded(ContentStore::embedded(), &players(2), POK, None, 1).unwrap();
        let names: Vec<String> = (0..9).map(|i| format!("p{i}")).collect();
        let r = Render::new(names.clone());
        let (res, seq) = spent_resolver();
        let mut cfg = res.core_cfg(&POK);
        cfg.initiative = names[..8].to_vec();
        assert_eq!(cfg_state(&r, &cfg, seq.peek()).map(|s| s.initiative.len()), Some(8));
        cfg.initiative = names;
        assert!(cfg_state(&r, &cfg, seq.peek()).is_none());
        let _ = state;
    }

    #[test]
    fn a_resolver_for_a_lent_window_takes_the_carried_part_and_the_clock_of_the_state() {
        let mut g = game(3, 7);
        play(&mut g, 80, 1000, |_| {});
        let state = g.state.clone();
        let r = render_for(&state);
        let (res, seq) = spent_resolver();
        let img = export(&res, &seq, &r).unwrap();
        let mut carried = to_state(&img).unwrap();
        let (lent, counter) = rebuild_for_lend(&state, &carried, Table::default(), &r);
        assert_eq!(counter.peek(), carried.next_id);
        assert_eq!(lent.used_keys().len(), carried.used.len());
        assert_eq!(lent.phase(), state.phase);
        assert_eq!(lent.counters(), (u64::from(state.round), u64::from(state.turn_seq)));
        assert_eq!(lent.speaker(), Some(&state.speaker), "the clock is the state's");
        assert!(lent.log().is_empty() && lent.applied_events().is_empty(), "an empty trace and journal");
        assert_eq!(lent.initiative_order().iter().map(|p| r.seat(p.as_str()).unwrap()).collect::<Vec<_>>(), carried.initiative.as_slice());
        // a carried part without an initiative order takes the state's own
        carried.initiative = ti4_engine_core::containers::SmallList::new();
        let (lent, _) = rebuild_for_lend(&state, &carried, Table::default(), &r);
        assert_eq!(lent.initiative_order(), state.initiative_order().as_slice());
    }

    /// What a step changes in the image, applied to the image before it, is the image after it.
    #[test]
    fn a_delta_applied_to_the_image_before_gives_the_image_after() {
        let mut g = game(4, 11);
        let mut changed = 0;
        for _ in 0..500 {
            if g.state.finished {
                break;
            }
            let r = render_for(&g.state);
            let before = export(&g.timing, g.event_sequence(), &r).unwrap();
            assert!(g.step().error.is_none());
            let after = export(&g.timing, g.event_sequence(), &r).unwrap();
            let d = delta(&before, &after);
            let mut applied = before.clone();
            apply(&mut applied, &d);
            // the clock and the seats are not part of a delta: they are re-derived from the state
            applied.seating.clone_from(&after.seating);
            (applied.active, applied.speaker, applied.phase) = (after.active, after.speaker, after.phase);
            (applied.round, applied.turn) = (after.round, after.turn);
            assert_eq!(applied, after);
            changed += usize::from(before != after);
        }
        assert!(changed > 0);
    }

    #[test]
    fn a_delta_keeps_the_common_prefix_and_replaces_a_rewritten_entry() {
        let r = Render::new(vec!["a".into(), "b".into()]);
        let used = |ids: &[(&str, u64)]| -> Vec<(AbilityId, UsedScope, u64)> {
            ids.iter().map(|(id, n)| (ability_id(&r, id), UsedScope::Round, *n)).collect()
        };
        let event = |id: u64, cancelled: bool| {
            let mut e = Event::new(id, "X", std::collections::BTreeMap::new());
            e.cancelled = cancelled;
            e
        };
        let base = ResolverImage {
            initiative: vec![Seat(0), Seat(1)],
            seating: vec![],
            active: None,
            speaker: None,
            phase: Phase::Action,
            round: 1,
            turn: 1,
            max_depth: 100,
            used: used(&[("ability:one", 1), ("ability:two", 1)]),
            emission_stack: vec![],
            applied: vec![event(1, false), event(2, false)],
            log: vec!["a".into(), "b".into()],
            next_event_id: 3,
        };
        let mut after = base.clone();
        after.used = used(&[("ability:one", 1), ("ability:three", 1)]);
        after.applied = vec![event(1, false), event(2, true), event(3, false)];
        after.log.push("c".into());
        after.next_event_id = 4;
        let d = delta(&base, &after);
        assert_eq!((d.used_keep, d.applied_keep, d.log_keep), (1, 1, 2));
        assert_eq!((d.used.len(), d.applied.len(), d.log.len()), (1, 2, 1));
        let mut applied = base;
        apply(&mut applied, &d);
        assert_eq!(applied, after);
    }
}
