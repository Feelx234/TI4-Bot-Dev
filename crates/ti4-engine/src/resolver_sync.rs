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
use ti4_engine_core::state::{EventKind, Slice};
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
        let mut spent = 0;
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 5, |g| {
                let r = render_for(&g.state);
                let mut seq = g.event_sequence().clone();
                let img = export(&g.timing, &seq, &r).unwrap();
                let rebuilt = rebuild(&g.state, &img, Table::default(), &r, Context::Image, &mut seq);
                assert_eq!(export(&rebuilt, &seq, &r).unwrap(), img);
                assert_eq!(seq.peek(), img.next_event_id);
                spent += img.used.len();
                checked += 1;
            });
        }
        assert!(checked > 400, "positions checked: {checked}");
        assert!(spent > 0, "no game spent a frequency key; the test would prove nothing about `used`");
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
        let mut triggers = 0;
        for (n, seed) in GAMES {
            let mut g = game(n, seed);
            play(&mut g, 600, 5, |g| {
                let r = render_for(&g.state);
                let seq = g.event_sequence().clone();
                let img = export(&g.timing, &seq, &r).unwrap();
                for (_, scope, id) in &img.used {
                    if *scope == UsedScope::Trigger {
                        assert!(*id < img.next_event_id, "a spent Trigger({id}) at counter {}", img.next_event_id);
                        triggers += 1;
                    }
                }
            });
        }
        assert!(triggers > 0, "no Trigger key was ever spent; the test proves nothing");
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
