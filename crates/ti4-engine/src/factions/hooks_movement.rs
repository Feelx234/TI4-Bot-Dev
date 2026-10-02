//! Faction hooks for movement and adjacency (`movement.rs`, `transit.rs`, `tactical.rs`): wormholes, move value, passing ships, out-of-turn movement, anomalies.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply. Tests install hooks with a
//! `#[cfg(test)] with_test_hooks(hooks, || ..)` that restores on panic, as in `hooks_combat.rs`.
//!
//! BF-00e-movement adds five hooks:
//!
//! | Hook | Card text it exists for | Call point |
//! |---|---|---|
//! | [`MovementHooks::extra_wormholes`] | Creuss flagship Hil Colish ("This ship's system contains a delta wormhole") | [`apply_extra_wormholes`], called by `MovementRules::with_laws` and by `laws::apply_to_galaxy` |
//! | [`MovementHooks::linked_systems`] | Creuss Quantum Entanglement, Winnu Lazax Gate Folding | `MovementRules::with_laws` and [`crate::movement::PlayerAdjacency`] |
//! | [`MovementHooks::move_bonus`] | Creuss Slipstream, Winnu breakthrough Imperator | `tactical::effective_move_value_*` |
//! | [`MovementHooks::may_move_through_ships`] | Mentak Corsair and Table's Grace, Yssaril flagship | `MovementRules::path_from_ship` |
//! | [`MovementHooks::may_enter_supernova`] | Muaat Gashlai Physiology | `MovementRules::with_laws` (`supernovae_open`) |
//!
//! The off-turn movement API (Naalu Foresight) is `transit::relocate_ships`, which emits the new
//! typed event `SHIPS_RELOCATED`; the wormhole token API is in `tokens.rs`; the map-edit API
//! (Creuss hero swap, Muaat hero supernova) is `movement::apply_map_edit`.

use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_content::units::UnitType;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::MODULES;

/// One ship about to have its move value read.
#[derive(Debug, Clone, Copy)]
pub struct MoveSite<'a> {
    /// The ship's owner (the moving player).
    pub player: &'a PlayerId,
    /// Where the ship starts its movement.
    pub origin: &'a SystemId,
    /// The ship's position in `state.ships_of(player, origin)` when the caller knows it. `None` from
    /// the entry points that do not carry it, so a hook keyed on one particular ship must return 0
    /// there and a caller that offers such a bonus must pass the index.
    pub index: Option<usize>,
    /// The ship's unit type.
    pub ship: &'a UnitType<'a>,
}

/// One ship type whose route may pass other players' ships.
#[derive(Debug, Clone, Copy)]
pub struct PassSite<'a> {
    /// The moving player.
    pub player: &'a PlayerId,
    /// The active system the ship is moving into.
    pub active: &'a SystemId,
    /// The unit type id of the moving ship (`mentak_cruiser3`, `yssaril_flagship`, ...).
    pub ship_type: &'a str,
}

/// Hooks for this area. Every field is optional; a module sets only what it needs.
#[derive(Debug, Clone, Copy)]
#[allow(
    clippy::type_complexity,
    reason = "plain fn-pointer table; aliases would only move the signatures elsewhere"
)]
pub struct MovementHooks {
    /// Wormholes this module's pieces put on the map **for every player**: `(system, kind)` with
    /// `kind` in the corpus's upper case (`"DELTA"`, `"ALPHA"`, ...).
    ///
    /// Creuss flagship: "This ship's system contains a delta wormhole." Derived from the state on
    /// every call and never stored, so it follows the ship. Called once per query, not per player;
    /// the result is merged with the printed and token wormholes before adjacency is read, so the
    /// law switches (Enforced Travel Ban etc.) apply to it like to any other wormhole.
    pub extra_wormholes: Option<fn(&GameState) -> Vec<(String, String)>>,
    /// Pairs of systems that are adjacent **for `player` only**, in both directions.
    ///
    /// Creuss Quantum Entanglement: "You treat all systems that contain either an alpha or beta
    /// wormhole as adjacent to each other. Game effects cannot prevent you from using this
    /// ability" (so this ignores `wormholes_off`; the hook sees the galaxy and builds the pairs
    /// from `Galaxy::wormhole_systems`/`wormhole_kinds`, which is unaffected by that switch).
    /// Winnu Lazax Gate Folding: "During your tactical actions, if you do not control Mecatol
    /// Rex, treat its system as if it has both an alpha and beta wormhole" (pairs Mecatol with
    /// every alpha/beta holder; the hook checks control and that a tactical action is under way).
    /// Called for every player; return nothing for anyone else.
    pub linked_systems: Option<
        fn(&GameState, &ContentStore, SourceSet, &Galaxy, &PlayerId) -> Vec<(String, String)>,
    >,
    /// Extra move value for one ship, added to the printed value (and to the action-card bonus)
    /// **before** Gravleash's anchor is applied.
    ///
    /// Creuss Slipstream: "Apply +1 to the move value of each of your ships that starts its
    /// movement in your home system or in a system that contains either an alpha or beta
    /// wormhole" (see [`wormholes_at`] and `Player::home_system`). Winnu Imperator: "After you
    /// activate a system that contains a legendary planet, apply +1 to the move value of 1 of
    /// your ships during this tactical action" (a one-ship choice: the module records which ship
    /// in `GameState::faction_marks` and matches on [`MoveSite::index`]). Summed over modules.
    pub move_bonus: Option<fn(&GameState, &MoveSite<'_>) -> i32>,
    /// Whether this ship type may move through systems containing other players' ships (58.4b is
    /// lifted for it). Any module's `true` allows. Called only for ship types the moving player
    /// owns on the board, once per `MovementRules` construction.
    ///
    /// Mentak Corsair: "If the active system contains another player's non-fighter ships, this unit
    /// can move through systems that contain other players' ships." Yssaril flagship: "This ship
    /// can move through systems that contain other player's ships."
    pub may_move_through_ships:
        Option<fn(&GameState, &ContentStore, SourceSet, &PassSite<'_>) -> bool>,
    /// Whether this player's ships may enter and move through supernovas (86.1 lifted, like the
    /// Magmus Reactor). Any module's `true` allows.
    ///
    /// Muaat Gashlai Physiology: "Your ships can move through supernovas."
    pub may_enter_supernova: Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId) -> bool>,
}

impl MovementHooks {
    /// No hooks.
    pub const NONE: Self = Self {
        extra_wormholes: None,
        linked_systems: None,
        move_bonus: None,
        may_move_through_ships: None,
        may_enter_supernova: None,
    };
}

// -- dispatch ------------------------------------------------------------------------------------

#[cfg(test)]
thread_local! {
    /// A hook table a test installs beside the (empty) module hooks. Per test thread.
    static TEST_HOOKS: std::cell::Cell<Option<MovementHooks>> =
        const { std::cell::Cell::new(None) };
}

/// Run `run` with `hooks` installed beside the module hooks on this thread, restoring the
/// previous table afterwards even if `run` panics.
#[cfg(test)]
pub(crate) fn with_test_hooks<T>(hooks: MovementHooks, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<MovementHooks>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_HOOKS.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(TEST_HOOKS.with(|cell| cell.replace(Some(hooks))));
    run()
}

#[cfg(test)]
fn test_table() -> Option<MovementHooks> {
    TEST_HOOKS.with(std::cell::Cell::get)
}

#[cfg(not(test))]
#[allow(clippy::unnecessary_wraps, reason = "same shape as the test build")]
const fn test_table() -> Option<MovementHooks> {
    None
}

/// Every module's movement hooks in [`MODULES`] order.
fn tables() -> impl Iterator<Item = MovementHooks> {
    MODULES
        .iter()
        .map(|module| module.hooks.movement)
        .chain(test_table())
}

/// Whether any module installs a hook of this kind; lets callers skip work entirely when none
/// does, which is what keeps games with empty modules identical.
pub(crate) fn any(pick: impl Fn(&MovementHooks) -> bool) -> bool {
    tables().any(|table| pick(&table))
}

/// Wormholes the modules' pieces put on the map, by system.
pub(crate) fn extra_wormholes(state: &GameState) -> BTreeMap<String, BTreeSet<String>> {
    extra_wormholes_by(tables(), state)
}

fn extra_wormholes_by(
    tables: impl Iterator<Item = MovementHooks>,
    state: &GameState,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for hook in tables.filter_map(|table| table.extra_wormholes) {
        for (system, kind) in hook(state) {
            found.entry(system).or_default().insert(kind);
        }
    }
    found
}

/// Merge the modules' wormholes into the galaxy's token wormholes. Additive and idempotent, so it
/// may run after `laws::apply_to_galaxy` has rebuilt the token map from the state. Returns whether
/// anything was added (so `MovementRules` clones the map only when it must).
pub fn apply_extra_wormholes(state: &GameState, galaxy: &mut Galaxy) -> bool {
    let mut changed = false;
    for (system, kinds) in extra_wormholes(state) {
        let held = galaxy.token_wormholes.entry(system).or_default();
        for kind in kinds {
            changed |= held.insert(kind);
        }
    }
    changed
}

/// Pairs adjacent for `player` only, summed over modules, normalised `(low, high)`, sorted.
pub(crate) fn linked_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    player: &PlayerId,
) -> Vec<(String, String)> {
    linked_systems_by(tables(), state, content, sources, galaxy, player)
}

fn linked_systems_by(
    tables: impl Iterator<Item = MovementHooks>,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    player: &PlayerId,
) -> Vec<(String, String)> {
    let mut pairs = BTreeSet::new();
    for hook in tables.filter_map(|table| table.linked_systems) {
        for (a, b) in hook(state, content, sources, galaxy, player) {
            if a != b {
                pairs.insert(if a < b { (a, b) } else { (b, a) });
            }
        }
    }
    pairs.into_iter().collect()
}

/// Summed per-ship move bonus.
pub(crate) fn move_bonus(state: &GameState, site: &MoveSite<'_>) -> i32 {
    move_bonus_by(tables(), state, site)
}

fn move_bonus_by(
    tables: impl Iterator<Item = MovementHooks>,
    state: &GameState,
    site: &MoveSite<'_>,
) -> i32 {
    tables
        .filter_map(|table| table.move_bonus)
        .map(|hook| hook(state, site))
        .sum()
}

/// Whether any module lets this ship type move through other players' ships.
pub(crate) fn may_move_through_ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    site: &PassSite<'_>,
) -> bool {
    may_move_through_ships_by(tables(), state, content, sources, site)
}

fn may_move_through_ships_by(
    tables: impl Iterator<Item = MovementHooks>,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    site: &PassSite<'_>,
) -> bool {
    tables
        .filter_map(|table| table.may_move_through_ships)
        .any(|hook| hook(state, content, sources, site))
}

/// Whether any module lets this player's ships enter supernovas.
pub(crate) fn may_enter_supernova(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> bool {
    may_enter_supernova_by(tables(), state, content, sources, player)
}

fn may_enter_supernova_by(
    tables: impl Iterator<Item = MovementHooks>,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> bool {
    tables
        .filter_map(|table| table.may_enter_supernova)
        .any(|hook| hook(state, content, sources, player))
}

// -- queries for faction modules -----------------------------------------------------------------

/// Wormhole kinds printed on tiles, by tile id, read once from the embedded corpus.
fn printed_wormholes() -> &'static BTreeMap<String, BTreeSet<String>> {
    static PRINTED: std::sync::OnceLock<BTreeMap<String, BTreeSet<String>>> =
        std::sync::OnceLock::new();
    PRINTED.get_or_init(|| {
        ti4_content::galaxy::all_systems(
            ContentStore::embedded(),
            ti4_model::content_types::DEFAULT,
        )
        .into_iter()
        .map(|(id, system)| {
            (
                id.to_owned(),
                system.wormholes().into_iter().map(str::to_owned).collect(),
            )
        })
        .collect()
    })
}

/// The wormhole kinds a system contains **now**, from the state alone: printed on the tile, plus
/// Creuss/gamma tokens, the ion storm's face, the opened Wormhole Nexus, and wormholes the
/// modules' pieces carry ([`MovementHooks::extra_wormholes`]).
///
/// For hooks that have no galaxy in hand (`move_bonus`). It reads the embedded corpus's printed
/// wormholes at [`ti4_model::content_types::DEFAULT`], which is what the game is built with; it
/// does not apply the law switches (a wormhole being present is not the same as it linking).
#[must_use]
pub fn wormholes_at(state: &GameState, system: &SystemId) -> BTreeSet<String> {
    let id = system.as_str();
    let mut kinds = printed_wormholes().get(id).cloned().unwrap_or_default();
    for (kind, at) in &state.wormhole_tokens {
        if at == system {
            kinds.insert(kind.clone());
        }
    }
    if let Some((at, face)) = &state.ion_storm
        && at == system
    {
        kinds.insert(face.clone());
    }
    if state.nexus_unlocked && id == crate::seating::LOCKED_NEXUS {
        kinds.insert("ALPHA".to_owned());
        kinds.insert("BETA".to_owned());
    }
    if let Some(extra) = extra_wormholes(state).get(id) {
        kinds.extend(extra.iter().cloned());
    }
    kinds
}

/// Whether a system holds an alpha or a beta wormhole now (see [`wormholes_at`]).
#[must_use]
pub fn has_alpha_or_beta(state: &GameState, system: &SystemId) -> bool {
    wormholes_at(state, system)
        .iter()
        .any(|kind| kind == "ALPHA" || kind == "BETA")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::content_types::DEFAULT;

    fn none() -> impl Iterator<Item = MovementHooks> {
        std::iter::once(MovementHooks::NONE)
    }

    #[test]
    fn empty_tables_are_neutral() {
        let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        let a = PlayerId::new("a");
        let galaxy = Galaxy::build(content, &["18", "19", "20"], DEFAULT, 1).unwrap();
        assert!(extra_wormholes_by(none(), &state).is_empty());
        assert!(linked_systems_by(none(), &state, content, DEFAULT, &galaxy, &a).is_empty());
        let origin = SystemId::new("01");
        let kind = ti4_content::units::unit_type(content, "carrier", DEFAULT).unwrap();
        let site = MoveSite {
            player: &a,
            origin: &origin,
            index: Some(0),
            ship: &kind,
        };
        assert_eq!(move_bonus_by(none(), &state, &site), 0);
        let pass = PassSite {
            player: &a,
            active: &origin,
            ship_type: "carrier",
        };
        assert!(!may_move_through_ships_by(
            none(),
            &state,
            content,
            DEFAULT,
            &pass
        ));
        assert!(!may_enter_supernova_by(
            none(),
            &state,
            content,
            DEFAULT,
            &a
        ));
        // The real module table is empty too.
        assert!(!any(|t| t.extra_wormholes.is_some()
            || t.linked_systems.is_some()
            || t.move_bonus.is_some()
            || t.may_move_through_ships.is_some()
            || t.may_enter_supernova.is_some()));
    }

    #[test]
    fn bonuses_sum_links_normalise_and_permissions_need_one_module() {
        let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        let a = PlayerId::new("a");
        let galaxy = Galaxy::build(content, &["18", "19", "20"], DEFAULT, 1).unwrap();
        let one = MovementHooks {
            move_bonus: Some(|_, _| 1),
            linked_systems: Some(|_, _, _, _, _| vec![("20".into(), "19".into())]),
            may_enter_supernova: Some(|_, _, _, _| true),
            ..MovementHooks::NONE
        };
        let two = MovementHooks {
            move_bonus: Some(|_, _| 2),
            linked_systems: Some(|_, _, _, _, _| {
                vec![("19".into(), "20".into()), ("19".into(), "19".into())]
            }),
            may_move_through_ships: Some(|_, _, _, site| site.ship_type == "cruiser"),
            ..MovementHooks::NONE
        };
        let all = || [one, two, MovementHooks::NONE].into_iter();
        let origin = SystemId::new("01");
        let kind = ti4_content::units::unit_type(content, "carrier", DEFAULT).unwrap();
        let site = MoveSite {
            player: &a,
            origin: &origin,
            index: None,
            ship: &kind,
        };
        assert_eq!(move_bonus_by(all(), &state, &site), 3);
        assert_eq!(
            linked_systems_by(all(), &state, content, DEFAULT, &galaxy, &a),
            vec![("19".to_owned(), "20".to_owned())],
            "both orders collapse to one normalised pair; a self-link is dropped"
        );
        assert!(may_enter_supernova_by(all(), &state, content, DEFAULT, &a));
        let pass = |ship_type| PassSite {
            player: &a,
            active: &origin,
            ship_type,
        };
        assert!(may_move_through_ships_by(
            all(),
            &state,
            content,
            DEFAULT,
            &pass("cruiser")
        ));
        assert!(!may_move_through_ships_by(
            all(),
            &state,
            content,
            DEFAULT,
            &pass("carrier")
        ));
    }

    #[test]
    fn wormholes_at_reads_tile_tokens_storm_nexus_and_module_wormholes() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol")], DEFAULT);
        let alpha_tile = SystemId::new("39");
        // Tile 39 prints an alpha wormhole in the corpus.
        assert!(
            wormholes_at(&state, &alpha_tile).contains("ALPHA"),
            "printed"
        );
        let plain = SystemId::new("19");
        assert!(wormholes_at(&state, &plain).is_empty());
        state
            .wormhole_tokens
            .insert("BETA".to_owned(), plain.clone());
        assert!(has_alpha_or_beta(&state, &plain), "a Creuss token");
        state.wormhole_tokens.clear();
        state.ion_storm = Some((plain.clone(), "ALPHA".to_owned()));
        assert!(has_alpha_or_beta(&state, &plain), "the ion storm's face");
        state.ion_storm = None;
        state.nexus_unlocked = true;
        let nexus = SystemId::new(crate::seating::LOCKED_NEXUS);
        assert!(has_alpha_or_beta(&state, &nexus), "the opened nexus");
        let flagship = MovementHooks {
            extra_wormholes: Some(|_| vec![("19".to_owned(), "DELTA".to_owned())]),
            ..MovementHooks::NONE
        };
        with_test_hooks(flagship, || {
            assert!(wormholes_at(&state, &plain).contains("DELTA"));
        });
        assert!(!wormholes_at(&state, &plain).contains("DELTA"), "restored");
    }

    #[test]
    fn apply_extra_wormholes_merges_once() {
        let state = crate::fixtures::seated_game(&[("a", "sol")], DEFAULT);
        let mut galaxy =
            Galaxy::build(ContentStore::embedded(), &["18", "19", "20"], DEFAULT, 1).unwrap();
        assert!(!apply_extra_wormholes(&state, &mut galaxy), "empty modules");
        let flagship = MovementHooks {
            extra_wormholes: Some(|_| vec![("19".to_owned(), "DELTA".to_owned())]),
            ..MovementHooks::NONE
        };
        with_test_hooks(flagship, || {
            assert!(apply_extra_wormholes(&state, &mut galaxy));
            assert!(!apply_extra_wormholes(&state, &mut galaxy), "idempotent");
        });
        assert!(galaxy.wormhole_kinds("19").contains("DELTA"));
    }
}
