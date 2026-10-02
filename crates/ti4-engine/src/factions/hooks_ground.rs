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
//! | `GROUND_FORCE_DESTROYED` | `system`, `planet`, `player` (owner), `unit` (type id), `damaged` (bool), `cause` | A ground force was destroyed by a hit **in an invasion** (`invasion.rs` paths only; ground forces destroyed by action/agenda cards or other rules elsewhere do not emit it). |
//! | `GROUND_COMBAT_STARTED` | `system`, `planet`, `attacker`, `defender` | Before the first round of a ground combat on a planet. |
//! | `GROUND_COMBAT_ENDED` | `system`, `planet`, `attacker`, `defender`, `winner` (absent if both sides were wiped out), `control_changed` (bool) | After the last round, before the next planet. |
//! | `GROUND_COMMITMENT_FINISHED` | `system`, `player`, `planets` (array of strings) | The invader has finished committing ground forces. |
//!
//! `cause` is one of `ground_combat`, `harrow`, `space_cannon_defense`, `bombardment`.
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
    /// Called with `(state, content, sources, invader, active_system, already)` each time the
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
            &PlayerId,
            &SystemId,
            &[CommitOrigin],
        ) -> Vec<CommitCandidate>,
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
    invader: &PlayerId,
    system: &SystemId,
    already: &[CommitOrigin],
) -> Vec<CommitCandidate> {
    hooks()
        .filter_map(|h| h.commit_candidates)
        .flat_map(|f| f(state, content, sources, invader, system, already))
        .collect()
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
