//! Faction hooks for invasion and ground combat (`invasion.rs`): bombardment, commitment, ground rounds, sustain, custodians.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply.
//!
//! # Typed events (BF-00c-ground)
//!
//! The invasion window emits these through `Resolving::emit`, so a faction's `timing_abilities`
//! can hang on them. All are *new* event types: no action card listens to any of them. Where the
//! payload carries `"system"`, `"planet"` and `"player"` they are plain strings.
//! They are emitted only by the live path (`InvasionWindow`); the synchronous test-only entry
//! points (`invasion::ground_combat`, `invasion::commit_ground_forces`, `invasion::resolve`
//! with no timing handle) emit nothing.
//!
//! | Event | Payload | When |
//! |---|---|---|
//! | `GROUND_FORCE_SUSTAINED` | `system`, `planet`, `player` (owner), `unit` (type id), `cause` | A ground force used SUSTAIN DAMAGE to cancel a hit. |
//! | `GROUND_FORCE_DESTROYED` | `system`, `planet`, `player` (owner), `unit` (type id), `damaged` (bool), `cause` | A ground force was destroyed: by a hit in an invasion (live `InvasionWindow`, emitted at once), or by an action card / agenda effect (staged, see below). |
//! | `PLANET_CONTROL_GAINED` | `system`, `planet`, `player` (new controller), `previous_owner` (absent if uncontrolled) | A player gained control of a planet: invasion landing (emitted at once), or an action card / agenda / legendary / Thunder's Edge effect (staged, see below). |
//! | `GROUND_COMBAT_STARTED` | `system`, `planet`, `attacker`, `defender` | Before the first round of a ground combat on a planet. |
//! | `GROUND_COMBAT_ENDED` | `system`, `planet`, `attacker`, `defender`, `winner` (absent if both sides were wiped out), `control_changed` (bool) | After the last round, before the next planet. |
//! | `GROUND_COMMITMENT_FINISHED` | `system`, `player`, `planets` (array of strings) | The invader has finished committing ground forces. |
//!
//! `cause` is one of `ground_combat`, `harrow`, `space_cannon_defense`, `bombardment` (invasion),
//! or `action_card:<id>` / `agenda:<id>` for a card or agenda effect.
//!
//! # Staged events (BF-F1)
//!
//! A card, agenda or legendary effect holds a `TimingContext` or only a `GameState`, never a
//! resolver, so it cannot emit. Such an effect calls [`stage_ground_force_destroyed`] /
//! [`stage_planet_control_gained`], which write one row per event into the invisible
//! `faction_marks` namespace `private:#staged:ground:` (no seat is named `#staged`, so
//! `mark_visible_to` shows it to nobody, and `cards:staged:` parsing is not disturbed). The
//! coordinator drains them in staging order with [`announce_staged_events`] (beside
//! `combat::announce_staged_destructions`) after every action card play, agenda resolution,
//! component action and leader action; [`has_staged_events`] tells whether anything waits. Until
//! drained the rows are in-flight bookkeeping.
//! Hits from a ground-combat round are resolved for both sides first (42.2: simultaneous) and the
//! events of the round are then emitted in the order the hits were applied, invader's casualties
//! last; a handler therefore sees the board after the whole round.
//!
//! `control_changed` is *predictive*: control is established at the end of the invasion (49.5), so
//! it says "the winner is the invader and did not control the planet", which is exactly what 49.5
//! will then do.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use super::CombatUnit;

/// A planet ground forces were committed *from* during this invasion, other than the active
/// system's space area.
pub type CommitOrigin = (SystemId, PlanetId);

/// One ground force a faction lets the invader commit from somewhere other than the space area
/// of the active system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitCandidate {
    /// The system the unit stands in.
    pub system: SystemId,
    /// The planet it stands on.
    pub planet: PlanetId,
    /// The unit, exactly as it stands there (type, owner, damage).
    pub unit: Unit,
}

/// Hooks for this area.
#[derive(Debug, Clone, Copy)]
#[allow(
    clippy::type_complexity,
    reason = "plain fn-pointer table; aliases would only move the signatures elsewhere"
)]
pub struct GroundHooks {
    /// Hits to add to one side's ground-combat roll.
    ///
    /// Sardakk `vpw` (Valkyrie Particle Weave): "After making combat rolls during a round of
    /// ground combat, if your opponent produced 1 or more hits, you produce 1 additional hit."
    ///
    /// Called once per side per round, after the roll *and* after the reroll windows (Fire Team,
    /// ...) have settled. Arguments after the planet: the roller's own hits, then the opponent's
    /// hits; both are the pre-hook values (X-89 already applied), computed before any module adds
    /// anything, so module order cannot matter. Returns the *additional* hits for the roller
    /// (summed over modules); the opponent assigns them like any other hit. Not called for Harrow
    /// or bombardment.
    pub ground_rolls_extra_hits: Option<
        fn(
            &GameState,
            &ContentStore,
            SourceSet,
            &PlayerId,
            &SystemId,
            &PlanetId,
            usize,
            usize,
        ) -> usize,
    >,
    /// Whether this ground force may use SUSTAIN DAMAGE now; every module must allow it.
    ///
    /// Mentak `mentak_mech`: "Other player's ground forces on this planet cannot use SUSTAIN
    /// DAMAGE." Called each time a hit looks for a unit to absorb it, for every undamaged ground
    /// force with SUSTAIN DAMAGE, with `context == "ground"` and `system`/`planet` set. Applies to
    /// every hit on ground forces in `invasion.rs` (rounds, Harrow, bombardment, space cannon
    /// defense). A unit that may not sustain is then simply not a candidate for absorbing.
    pub may_sustain: Option<fn(&GameState, &ContentStore, SourceSet, &CombatUnit<'_>) -> bool>,
    /// Extra sources of ground forces the invader may commit.
    ///
    /// Sardakk `sardakkcommander`: "You can commit (move) up to 1 ground force from each planet
    /// in the active system and each planet in adjacent systems that do not contain 1 of your
    /// command tokens."
    ///
    /// Called with `(state, content, sources, galaxy, invader, active_system, already)` each time the
    /// commit question is built or answered; `already` lists the planets this invasion has
    /// already committed from through this hook, so "up to 1 from each planet" is expressible.
    /// The hook returns candidates; the engine drops any that are not really the invader's
    /// ground force on that planet, offers them as `commit|<n>|<planet>` options after the
    /// ordinary ones (so existing option ids never change) carrying `from_system` and
    /// `from_planet`, and performs the move: remove from the origin planet, place on the target.
    /// Moving a unit onto the planet it already stands on is not offered. Legality of the source
    /// (command tokens, adjacency, count limits) is the hook's.
    pub commit_candidates: Option<
        fn(
            &GameState,
            &ContentStore,
            SourceSet,
            Option<&ti4_content::galaxy::Galaxy>,
            &PlayerId,
            &SystemId,
            &[CommitOrigin],
        ) -> Vec<CommitCandidate>,
    >,
    /// Extra units from the active system's space area that may commit temporarily as ground
    /// forces. The invasion returns surviving candidates to space before it establishes control.
    ///
    /// Naalu's Matriarch supplies its fighters here. Unlike [`Self::commit_candidates`], these
    /// units already stand in the active system's space area, so the engine validates that they
    /// are still there and keeps their temporary provenance through the invasion.
    pub temporary_space_commit_candidates:
        Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId, &SystemId) -> Vec<Unit>>,
    /// Whether a unit temporarily counts as a ground force on a planet during an invasion.
    pub temporary_ground_force: Option<
        fn(&GameState, &ContentStore, SourceSet, &PlayerId, &SystemId, &PlanetId, &Unit) -> bool,
    >,
    /// At the start of a round of ground combat, once for each participant (the invader, then
    /// the defender), before anything is rolled.
    ///
    /// Naaz `sc` (Supercharge): "At the start of a combat round, ..." is the ground half of
    /// `Hooks::space_combat_round_started`. Arguments after the table: the participant, the
    /// system and the planet. Called in the live window and in the synchronous `ground_combat`;
    /// a participant may have lost every ground force before a later module runs, so check the
    /// board. Refusal means doing nothing; the hook must be atomic.
    pub ground_combat_round_started: Option<
        fn(
            &mut GameState,
            &ContentStore,
            SourceSet,
            &mut crate::choice::Table,
            &PlayerId,
            &SystemId,
            &PlanetId,
        ),
    >,
    /// Whether this player need not spend influence to remove the custodians token.
    ///
    /// Winnu `blood_ties`: "You are not required to spend influence to remove the custodians
    /// token." Consulted where the removal is offered, previewed and paid (27.2). The other
    /// requirement (27.2a, ground forces that can land on Mecatol Rex) still applies.
    pub custodians_free: Option<fn(&GameState, &ContentStore, &PlayerId) -> bool>,
}

impl GroundHooks {
    /// No hooks.
    pub const NONE: Self = Self {
        ground_rolls_extra_hits: None,
        may_sustain: None,
        commit_candidates: None,
        temporary_space_commit_candidates: None,
        temporary_ground_force: None,
        ground_combat_round_started: None,
        custodians_free: None,
    };
}

fn hooks() -> impl Iterator<Item = GroundHooks> {
    let modules = super::MODULES.iter().map(|module| module.hooks.ground);
    // Tests install one extra hook set (after every module), since `MODULES` is a constant.
    #[cfg(test)]
    let modules = modules.chain(test_support::installed());
    modules
}

// -- dispatch, called from invasion.rs ------------------------------------------------------------

pub(crate) fn ground_rolls_extra_hits(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
    opponent_hits: usize,
) -> usize {
    hooks()
        .filter_map(|h| h.ground_rolls_extra_hits)
        .map(|f| {
            f(
                state,
                content,
                sources,
                player,
                system,
                planet,
                hits,
                opponent_hits,
            )
        })
        .sum()
}

pub(crate) fn may_sustain(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    hooks()
        .filter_map(|h| h.may_sustain)
        .all(|f| f(state, content, sources, unit))
}

pub(crate) fn commit_candidates(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    invader: &PlayerId,
    system: &SystemId,
    already: &[CommitOrigin],
) -> Vec<CommitCandidate> {
    hooks()
        .filter_map(|h| h.commit_candidates)
        .flat_map(|f| f(state, content, sources, galaxy, invader, system, already))
        .collect()
}

pub(crate) fn temporary_space_commit_candidates(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    invader: &PlayerId,
    system: &SystemId,
) -> Vec<Unit> {
    hooks()
        .filter_map(|h| h.temporary_space_commit_candidates)
        .flat_map(|f| f(state, content, sources, invader, system))
        .collect()
}

pub(crate) fn temporary_ground_force(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    unit: &Unit,
) -> bool {
    hooks()
        .filter_map(|h| h.temporary_ground_force)
        .any(|f| f(state, content, sources, player, system, planet, unit))
}

pub(crate) fn ground_combat_round_started(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut crate::choice::Table,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) {
    for f in hooks().filter_map(|h| h.ground_combat_round_started) {
        f(state, content, sources, table, player, system, planet);
    }
}

// -- staged events (resolver-less effects) -------------------------------------------------------

/// `faction_marks` namespace holding staged ground events. Invisible to every seat.
const STAGED: &str = "private:#staged:ground:";

fn stage(state: &mut GameState, row: String) {
    let next = state
        .faction_marks
        .range(STAGED.to_owned()..)
        .map_while(|(key, _)| key.strip_prefix(STAGED))
        .filter_map(|n| n.parse::<u64>().ok())
        .max()
        .map_or(0, |n| n + 1);
    state
        .faction_marks
        .insert(format!("{STAGED}{next:08}"), row);
}

/// Stage `GROUND_FORCE_DESTROYED` for `unit`, which the caller has already removed from `planet`.
/// `cause` must not contain `|`.
pub(crate) fn stage_ground_force_destroyed(
    state: &mut GameState,
    system: &SystemId,
    planet: &PlanetId,
    unit: &Unit,
    cause: &str,
) {
    stage(
        state,
        format!(
            "destroyed|{system}|{planet}|{}|{}|{}|{cause}",
            unit.owner, unit.type_id, unit.sustained_damage
        ),
    );
}

/// Stage `PLANET_CONTROL_GAINED`, which the caller has already applied. The announcement also
/// records the capture in `last_control_gained`, as an invasion landing does.
pub(crate) fn stage_planet_control_gained(
    state: &mut GameState,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    previous: Option<&PlayerId>,
) {
    stage(
        state,
        format!(
            "control|{system}|{planet}|{player}|{}",
            previous.map_or_else(String::new, ToString::to_string)
        ),
    );
}

/// Whether any staged ground event waits to be announced.
#[must_use]
pub fn has_staged_events(state: &GameState) -> bool {
    state
        .faction_marks
        .range(STAGED.to_owned()..)
        .next()
        .is_some_and(|(key, _)| key.starts_with(STAGED))
}

/// Emit every staged `GROUND_FORCE_DESTROYED` / `PLANET_CONTROL_GAINED`, in staging order, and
/// remove the rows. An announcement may stage more; this runs until nothing is left. Cancels are
/// ignored (the thing has happened). A no-op when nothing is staged.
///
/// **Flush site for the coordinator:** call it where `combat::announce_staged_destructions` is
/// called (`Game::announce_staged_destructions`), and also after an action card resolves and after
/// an agenda outcome resolves.
pub fn announce_staged_events(state: &mut GameState, ctx: &mut crate::choice::Resolving<'_>) {
    while has_staged_events(state) {
        let keys: Vec<String> = state
            .faction_marks
            .range(STAGED.to_owned()..)
            .map_while(|(key, _)| key.starts_with(STAGED).then(|| key.clone()))
            .collect();
        for key in keys {
            let Some(row) = state.faction_marks.remove(&key) else {
                continue;
            };
            let parts: Vec<&str> = row.split('|').collect();
            let mut payload = std::collections::BTreeMap::new();
            let name = match parts[..] {
                ["destroyed", system, planet, player, unit, damaged, cause] => {
                    payload.insert("system".to_owned(), system.into());
                    payload.insert("planet".to_owned(), planet.into());
                    payload.insert("player".to_owned(), player.into());
                    payload.insert("unit".to_owned(), unit.into());
                    payload.insert("damaged".to_owned(), (damaged == "true").into());
                    payload.insert("cause".to_owned(), cause.into());
                    "GROUND_FORCE_DESTROYED"
                }
                ["control", system, planet, player, previous] => {
                    payload.insert("system".to_owned(), system.into());
                    payload.insert("planet".to_owned(), planet.into());
                    payload.insert("player".to_owned(), player.into());
                    if !previous.is_empty() {
                        payload.insert("previous_owner".to_owned(), previous.into());
                    }
                    state.last_control_gained = Some((
                        SystemId::new(system),
                        PlanetId::new(planet),
                        PlayerId::new(player),
                        (!previous.is_empty()).then(|| PlayerId::new(previous)),
                    ));
                    "PLANET_CONTROL_GAINED"
                }
                _ => continue,
            };
            let _ = ctx.emit(state, name, payload);
        }
    }
}

/// Deliver staged ground events with per-event rollback and retry on reaction failure.
///
/// # Errors
/// Returns a timing error while preserving the failed row, reaction state, dice, RNG and journal.
pub fn try_announce_staged_events(
    state: &mut GameState,
    ctx: &mut crate::choice::Resolving<'_>,
) -> Result<(), crate::timing::TimingError> {
    if ctx.timing.is_none() {
        return Ok(());
    }
    while has_staged_events(state) {
        let keys: Vec<String> = state
            .faction_marks
            .range(STAGED.to_owned()..)
            .map_while(|(key, _)| key.starts_with(STAGED).then(|| key.clone()))
            .collect();
        for key in keys {
            let before_state = state.clone();
            let before_dice = ctx.dice.clone();
            let before_rng = ctx.rng.clone();
            let before_timing = ctx
                .timing
                .as_ref()
                .map(|handle| (handle.sequence.clone(), handle.resolver.checkpoint()));
            let Some(row) = state.faction_marks.remove(&key) else {
                continue;
            };
            let parts: Vec<&str> = row.split('|').collect();
            let mut payload = std::collections::BTreeMap::new();
            let name = match parts[..] {
                ["destroyed", system, planet, player, unit, damaged, cause] => {
                    payload.insert("system".to_owned(), system.into());
                    payload.insert("planet".to_owned(), planet.into());
                    payload.insert("player".to_owned(), player.into());
                    payload.insert("unit".to_owned(), unit.into());
                    payload.insert("damaged".to_owned(), (damaged == "true").into());
                    payload.insert("cause".to_owned(), cause.into());
                    "GROUND_FORCE_DESTROYED"
                }
                ["control", system, planet, player, previous] => {
                    payload.insert("system".to_owned(), system.into());
                    payload.insert("planet".to_owned(), planet.into());
                    payload.insert("player".to_owned(), player.into());
                    if !previous.is_empty() {
                        payload.insert("previous_owner".to_owned(), previous.into());
                    }
                    state.last_control_gained = Some((
                        SystemId::new(system),
                        PlanetId::new(planet),
                        PlayerId::new(player),
                        (!previous.is_empty()).then(|| PlayerId::new(previous)),
                    ));
                    "PLANET_CONTROL_GAINED"
                }
                _ => continue,
            };
            if let Err(error) = ctx.emit(state, name, payload) {
                *state = before_state;
                *ctx.dice = before_dice;
                *ctx.rng = before_rng;
                if let (Some(handle), Some((sequence, resolver))) =
                    (ctx.timing.as_mut(), before_timing)
                {
                    *handle.sequence = sequence;
                    handle.resolver.restore(resolver);
                }
                return Err(error);
            }
        }
    }
    Ok(())
}

pub(crate) fn custodians_free(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> bool {
    hooks()
        .filter_map(|h| h.custodians_free)
        .any(|f| f(state, content, player))
}

#[cfg(test)]
pub(crate) use test_support::with_test_hooks;

#[cfg(test)]
pub(crate) mod test_support {
    use super::GroundHooks;
    use std::cell::Cell;

    thread_local! {
        static INSTALLED: Cell<Option<GroundHooks>> = const { Cell::new(None) };
    }

    pub(super) fn installed() -> Option<GroundHooks> {
        INSTALLED.with(Cell::get)
    }

    /// Flush the staged ground events of `state` through a resolver that records every
    /// `GROUND_FORCE_DESTROYED` and `PLANET_CONTROL_GAINED` it emits, in order.
    #[allow(
        clippy::let_and_return,
        reason = "the guard must drop before `seen` does"
    )]
    pub(crate) fn flush_recorded(
        state: &mut ti4_model::state::GameState,
    ) -> Vec<(
        String,
        std::collections::BTreeMap<String, serde_json::Value>,
    )> {
        use crate::timing::{Ability, Relation, Resolver};
        use std::sync::{Arc, Mutex};
        use ti4_model::id::PlayerId;
        let seen: Arc<Mutex<Vec<_>>> = Arc::default();
        let who = PlayerId::new("a");
        let mut resolver = Resolver::new(
            vec![who.clone()],
            Some(who.clone()),
            crate::choice::Table::default(),
        );
        resolver.register(
            ["GROUND_FORCE_DESTROYED", "PLANET_CONTROL_GAINED"]
                .into_iter()
                .map(|event| {
                    let seen = Arc::clone(&seen);
                    Ability::new(
                        format!("test:{event}"),
                        who.clone(),
                        event,
                        Relation::After,
                        Arc::new(move |event, _| {
                            seen.lock()
                                .unwrap()
                                .push((event.event_type.clone(), event.payload.clone()));
                            Ok(())
                        }),
                    )
                }),
        );
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(0);
        let mut table = crate::choice::Table::default();
        let mut sequence = crate::event::EventSequence::new();
        {
            let mut ctx = crate::choice::Resolving {
                content: ti4_content::ContentStore::embedded(),
                sources: ti4_model::content_types::POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle {
                    resolver: &mut resolver,
                    sequence: &mut sequence,
                    galaxy: None,
                }),
            };
            super::announce_staged_events(state, &mut ctx);
        }
        let out = seen.lock().unwrap().clone();
        out
    }

    /// Run `run` with `hooks` dispatched after every module's. The previous value is restored
    /// afterwards, even on panic.
    pub(crate) fn with_test_hooks<T>(hooks: GroundHooks, run: impl FnOnce() -> T) -> T {
        struct Restore(Option<GroundHooks>);
        impl Drop for Restore {
            fn drop(&mut self) {
                INSTALLED.with(|cell| cell.set(self.0));
            }
        }
        let _restore = Restore(INSTALLED.with(|cell| cell.replace(Some(hooks))));
        run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::view::mark_visible_to;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    #[test]
    fn staged_events_are_announced_in_order_once_and_are_invisible() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        assert!(!has_staged_events(&state));
        assert!(test_support::flush_recorded(&mut state).is_empty());

        let (system, planet) = (SystemId::new("18"), PlanetId::new("mr"));
        let unit = Unit::new(ti4_model::id::UnitTypeId::new("infantry"), a());
        stage_ground_force_destroyed(&mut state, &system, &planet, &unit, "action_card:plague");
        stage_planet_control_gained(
            &mut state,
            &system,
            &planet,
            &PlayerId::new("b"),
            Some(&a()),
        );
        stage_planet_control_gained(&mut state, &system, &planet, &a(), None);
        assert!(has_staged_events(&state));
        assert!(
            state.faction_marks.keys().all(
                |key| !mark_visible_to(key, &a()) && !mark_visible_to(key, &PlayerId::new("b"))
            )
        );

        let events = test_support::flush_recorded(&mut state);
        let names: Vec<&str> = events.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "GROUND_FORCE_DESTROYED",
                "PLANET_CONTROL_GAINED",
                "PLANET_CONTROL_GAINED"
            ]
        );
        let destroyed = &events[0].1;
        assert_eq!(destroyed["system"], "18");
        assert_eq!(destroyed["planet"], "mr");
        assert_eq!(destroyed["player"], "a");
        assert_eq!(destroyed["unit"], "infantry");
        assert_eq!(destroyed["damaged"], false);
        assert_eq!(destroyed["cause"], "action_card:plague");
        assert_eq!(events[1].1["player"], "b");
        assert_eq!(events[1].1["previous_owner"], "a");
        assert!(!events[2].1.contains_key("previous_owner"));
        assert!(!has_staged_events(&state), "drained");
        assert!(state.faction_marks.is_empty());
        assert!(test_support::flush_recorded(&mut state).is_empty(), "once");
    }

    #[test]
    fn the_new_hooks_are_neutral_when_empty() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        let before = state.clone();
        let mut table = crate::choice::Table::default();
        ground_combat_round_started(
            &mut state,
            ti4_content::ContentStore::embedded(),
            ti4_model::content_types::POK,
            &mut table,
            &a(),
            &SystemId::new("18"),
            &PlanetId::new("mr"),
        );
        assert_eq!(state, before);
        assert!(
            commit_candidates(
                &state,
                ti4_content::ContentStore::embedded(),
                ti4_model::content_types::POK,
                None,
                &a(),
                &SystemId::new("18"),
                &[],
            )
            .is_empty()
        );
    }
}
