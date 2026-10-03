//! Faction hooks for space combat (`combat.rs`): rounds, hits, sustain, destruction, retreat, AFB.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply.
//!
//! BF-00c-space adds four hooks and the typed events they sit beside:
//!
//! | Hook | Card text it exists for | Call point |
//! |---|---|---|
//! | [`CombatHooks::produced_hits`] | Mentak Ambush, Yin Impulse Core, Yin Devotion | `CombatMoment` below |
//! | [`CombatHooks::may_sustain`] | Mentak flagship | every space SUSTAIN DAMAGE offer |
//! | [`CombatHooks::direct_hit_immune`] | Exotrireme II | the `direct_hittable` payload of `SUSTAIN_DAMAGE_USED` |
//! | [`CombatHooks::afb_excess`] | Argent Raid Formation | a barrage hit with no fighter left to take it |
//!
//! Events (emitted from `combat.rs`, consumed through `Hooks::timing_abilities`):
//! `SPACE_COMBAT_ROUND_ENDED` and `SPACE_COMBAT_ENDED`.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::{CombatUnit, MODULES};
use crate::choice::IllegalChoice;
use crate::timing::TimingContext;

/// When in a space combat a module may produce hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatMoment {
    /// "At the start of a space combat": after Assault Cannon, **before** anti-fighter barrage.
    /// The hits are assigned at once, before any die is rolled, and the round then resumes at
    /// the barrage.
    /// Mentak Ambush ("You may roll 1 die for each of up to 2 of your cruisers or destroyers in
    /// the system. For each result equal to or greater than that ship's combat value, produce 1
    /// hit; your opponent must assign it to 1 of their ships") and Yin Impulse Core ("At the
    /// start of a space combat, you may destroy 1 of your cruisers or destroyers in the active
    /// system to produce 1 hit against your opponent's ships; that hit must be assigned by your
    /// opponent to 1 of their non-fighters ships if able").
    CombatStart,
    /// After this player's dice, and the opponent's, are rolled and rerolled, and before any hit
    /// lands: the produced hits join the round's own simultaneous hits (78.6).
    AfterRoll,
    /// "After each space combat round": once the round is over, retreats included (78.7), before
    /// `SPACE_COMBAT_ROUND_ENDED` and the next round. Not asked when the combat is already over.
    /// Yin Devotion ("You may destroy 1 of your cruisers or destroyers in the active system to
    /// produce 1 hit and assign it to 1 of your opponent's ships in that system"). The hits are
    /// assigned immediately.
    RoundEnded,
}

/// Who is producing hits, against whom, and when.
#[derive(Debug, Clone, Copy)]
pub struct HitSite<'a> {
    /// The hit producer, the participant the hook is being asked about.
    pub player: &'a PlayerId,
    /// The producer's opponent, who assigns the hits.
    pub opponent: &'a PlayerId,
    /// The system the combat is in (the active system).
    pub system: &'a SystemId,
    /// The round (1-based). [`CombatMoment::CombatStart`] is reported as round 1.
    pub round: u32,
    /// The moment.
    pub moment: CombatMoment,
}

/// Hits a module produced for one participant.
///
/// They are assigned by the opponent exactly as ordinary combat hits: SUSTAIN DAMAGE offered
/// first (87.1), then a casualty chosen by the owner (78.6). `producer` of the hit is the
/// producing player, so Direct Hit keys on it as it does for dice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProducedHits {
    /// Hits the opponent may assign to any of their ships.
    pub any_ship: usize,
    /// Hits the **producer** assigns, to any of the opponent's ships (Devotion: "produce 1 hit
    /// and assign it to 1 of your opponent's ships"). The producer is asked which ship; the
    /// owner may still use SUSTAIN DAMAGE where legal.
    pub producer_assigned: usize,
    /// Hits the opponent must assign to a non-fighter ship if able (Impulse Core).
    pub non_fighter: usize,
}

impl ProducedHits {
    /// No hits.
    pub const NONE: Self = Self {
        any_ship: 0,
        producer_assigned: 0,
        non_fighter: 0,
    };

    /// Both kinds together.
    #[must_use]
    pub const fn total(self) -> usize {
        self.any_ship + self.producer_assigned + self.non_fighter
    }

    const fn plus(self, other: Self) -> Self {
        Self {
            any_ship: self.any_ship + other.any_ship,
            producer_assigned: self.producer_assigned + other.producer_assigned,
            non_fighter: self.non_fighter + other.non_fighter,
        }
    }
}

/// Anti-fighter barrage hits that found no fighter left to destroy.
#[derive(Debug, Clone, Copy)]
pub struct AfbExcess<'a> {
    /// The player whose barrage produced the hits.
    pub producer: &'a PlayerId,
    /// The player the barrage was fired at.
    pub target: &'a PlayerId,
    /// The system the combat is in.
    pub system: &'a SystemId,
    /// Hits produced beyond the target's fighters (15.2a would otherwise discard them).
    pub excess: usize,
}

/// Hooks for this area. Every field is optional; a module sets only what it needs.
#[derive(Debug, Clone, Copy)]
#[allow(
    clippy::type_complexity,
    reason = "plain fn-pointer table; aliases would only move the signatures elsewhere"
)]
pub struct CombatHooks {
    /// Hits this module's player produces at `site.moment` (see [`CombatMoment`]).
    ///
    /// Called for **every** participant and every module, `site.player` first the attacker then
    /// the defender. A hook that "may" produce hits asks its own question through the context's
    /// table; dice come from `context.dice` with a reason label, never another source. The hook
    /// pays its own costs (it destroys its own cruiser through `crate::combat::destroy_units`,
    /// which stages the `SHIP_DESTROYED` announcement the combat then drains) before returning
    /// the hits; returning [`ProducedHits::NONE`] leaves the state as it was. An illegal answer
    /// from a decider is returned as `Err` and fails the combat step like any other combat choice.
    pub produced_hits:
        Option<fn(&mut TimingContext<'_>, &HitSite<'_>) -> Result<ProducedHits, IllegalChoice>>,
    /// Whether this unit may use SUSTAIN DAMAGE against a space-combat hit now. **Every** module
    /// must allow; any `false` forbids it. `unit.player` is the sustaining unit's owner,
    /// `unit.system` the combat system, `unit.context` is `"space"`.
    ///
    /// Mentak flagship: "Other player's ships in this system cannot use SUSTAIN DAMAGE." The
    /// ground path (Mentak mech, "Other player's ground forces on this planet cannot use SUSTAIN
    /// DAMAGE") is a separate gate in `invasion.rs`: see `plans/evidence/BF-00c-space.md`.
    pub may_sustain: Option<fn(&GameState, &ContentStore, SourceSet, &CombatUnit<'_>) -> bool>,
    /// Whether a ship of `unit` is immune to "Direct Hit" action cards, **on top of** the
    /// printed `canBeDirectHit` flag. Any module's `true` makes it immune. Read at every
    /// `SUSTAIN_DAMAGE_USED` (the card's window guard) through
    /// `crate::combat::direct_hittable_at`.
    pub direct_hit_immune:
        Option<fn(&GameState, &ContentStore, SourceSet, &CombatUnit<'_>) -> bool>,
    /// Anti-fighter barrage hits in excess of the target's fighters.
    ///
    /// Argent Raid Formation: "When 1 or more of your units uses ANTI-FIGHTER BARRAGE: for each
    /// hit produced in excess of your opponent's fighters, choose 1 of your opponent's ships that
    /// has SUSTAIN DAMAGE to become damaged." Called after the fighters were destroyed, for the
    /// barrage's producer only if `excess > 0`. Not called for a Waylay barrage (those hits are
    /// ordinary combat hits against all ships, so nothing is in excess).
    pub afb_excess: Option<fn(&mut TimingContext<'_>, &AfbExcess<'_>) -> Result<(), IllegalChoice>>,
    /// Whether `shooter` may not use SPACE CANNON against `target_owner`'s ships in `system` (the
    /// active system). **Any** module's `true` bars; read by `combat::space_cannon_offense` for
    /// guns in the system and for adjacent-reaching guns alike.
    ///
    /// Argent flagship: "Other players cannot use SPACE CANNON against your ships in this
    /// system." The module checks that `target_owner` has the flagship in `system` and that
    /// `shooter` is another player.
    pub space_cannon_barred:
        Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId, &PlayerId, &SystemId) -> bool>,
    /// Whether `player`'s excess fighters of `unit_type` count as half a ship against the fleet
    /// pool. Any module's `true` halves; read by `fleet::standing`.
    ///
    /// Naalu Hybrid Crystal Fighter II: "Fighters in excess of your ships' capacity count as 1/2
    /// of a ship against your fleet pool." The module checks `player` is Naalu and `unit_type` is
    /// `naalu_fighter2`.
    pub fighter_fleet_weight_halves: Option<fn(&GameState, &ContentStore, &PlayerId, &str) -> bool>,
}

impl CombatHooks {
    /// No hooks.
    pub const NONE: Self = Self {
        produced_hits: None,
        may_sustain: None,
        direct_hit_immune: None,
        afb_excess: None,
        space_cannon_barred: None,
        fighter_fleet_weight_halves: None,
    };
}

// -- dispatch ------------------------------------------------------------------------------------

#[cfg(test)]
thread_local! {
    /// A hook table a test installs beside the (empty) module hooks, so the call sites in
    /// `combat.rs` can be exercised before any faction registers one. Per test thread.
    static TEST_HOOKS: std::cell::Cell<Option<CombatHooks>> =
        const { std::cell::Cell::new(None) };
}

/// Run `run` with `hooks` installed beside the module hooks on this thread, restoring the
/// previous table afterwards even if `run` panics.
#[cfg(test)]
pub(crate) fn with_test_hooks<T>(hooks: CombatHooks, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<CombatHooks>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_HOOKS.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(TEST_HOOKS.with(|cell| cell.replace(Some(hooks))));
    run()
}

#[cfg(test)]
fn test_table() -> Option<CombatHooks> {
    TEST_HOOKS.with(std::cell::Cell::get)
}

#[cfg(not(test))]
#[allow(clippy::unnecessary_wraps, reason = "same shape as the test build")]
const fn test_table() -> Option<CombatHooks> {
    None
}

/// Every module's combat hooks in [`MODULES`] order.
fn tables() -> impl Iterator<Item = CombatHooks> {
    MODULES
        .iter()
        .map(|module| module.hooks.combat)
        .chain(test_table())
}

/// Hits produced at `site.moment`, summed over every module in order.
pub(crate) fn produced_hits(
    context: &mut TimingContext<'_>,
    site: &HitSite<'_>,
) -> Result<ProducedHits, IllegalChoice> {
    produced_hits_by(tables(), context, site)
}

fn produced_hits_by(
    tables: impl Iterator<Item = CombatHooks>,
    context: &mut TimingContext<'_>,
    site: &HitSite<'_>,
) -> Result<ProducedHits, IllegalChoice> {
    let mut sum = ProducedHits::NONE;
    for hook in tables.filter_map(|table| table.produced_hits) {
        sum = sum.plus(hook(context, site)?);
    }
    Ok(sum)
}

/// Whether every module lets this unit use SUSTAIN DAMAGE in space.
pub(crate) fn may_sustain(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    may_sustain_by(tables(), state, content, sources, unit)
}

fn may_sustain_by(
    mut tables: impl Iterator<Item = CombatHooks>,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    tables
        .by_ref()
        .filter_map(|table| table.may_sustain)
        .all(|hook| hook(state, content, sources, unit))
}

/// Whether any module makes this unit immune to Direct Hit.
pub(crate) fn direct_hit_immune(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    direct_hit_immune_by(tables(), state, content, sources, unit)
}

fn direct_hit_immune_by(
    mut tables: impl Iterator<Item = CombatHooks>,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    tables
        .by_ref()
        .filter_map(|table| table.direct_hit_immune)
        .any(|hook| hook(state, content, sources, unit))
}

/// Tell every module about barrage hits that found no fighter.
pub(crate) fn afb_excess(
    context: &mut TimingContext<'_>,
    site: &AfbExcess<'_>,
) -> Result<(), IllegalChoice> {
    for hook in tables().filter_map(|table| table.afb_excess) {
        hook(context, site)?;
    }
    Ok(())
}

/// Whether any module bars `shooter`'s SPACE CANNON against `target_owner`'s ships in `system`.
pub(crate) fn space_cannon_barred(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    shooter: &PlayerId,
    target_owner: &PlayerId,
    system: &SystemId,
) -> bool {
    tables()
        .filter_map(|table| table.space_cannon_barred)
        .any(|hook| hook(state, content, sources, shooter, target_owner, system))
}

/// Whether any module makes `player`'s excess `unit_type` fighters weigh half a ship.
pub(crate) fn fighter_fleet_weight_halves(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
    unit_type: &str,
) -> bool {
    tables()
        .filter_map(|table| table.fighter_fleet_weight_halves)
        .any(|hook| hook(state, content, player, unit_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::content_types::DEFAULT;

    fn unit<'a>(player: &'a PlayerId, system: &'a SystemId) -> CombatUnit<'a> {
        CombatUnit {
            player,
            system: Some(system),
            planet: None,
            unit_type: "cruiser",
            context: "space",
        }
    }

    #[test]
    fn empty_tables_are_neutral() {
        let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        let (a, system) = (PlayerId::new("a"), SystemId::new("18"));
        let unit = unit(&a, &system);
        let none = || std::iter::once(CombatHooks::NONE);
        assert!(may_sustain_by(none(), &state, content, DEFAULT, &unit));
        assert!(!direct_hit_immune_by(
            none(),
            &state,
            content,
            DEFAULT,
            &unit
        ));
        let mut state = state;
        let mut table = crate::choice::Table::default();
        let (b, system) = (PlayerId::new("b"), SystemId::new("18"));
        let site = HitSite {
            player: &a,
            opponent: &b,
            system: &system,
            round: 1,
            moment: CombatMoment::CombatStart,
        };
        let hits = crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            produced_hits_by(none(), ctx, &site)
        });
        assert_eq!(hits, Ok(ProducedHits::NONE));
    }

    #[test]
    fn sustain_needs_every_module_and_immunity_needs_one() {
        let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        let (a, system) = (PlayerId::new("a"), SystemId::new("18"));
        let unit = unit(&a, &system);
        let allow = CombatHooks {
            may_sustain: Some(|_, _, _, _| true),
            direct_hit_immune: Some(|_, _, _, _| false),
            ..CombatHooks::NONE
        };
        let forbid = CombatHooks {
            may_sustain: Some(|_, _, _, _| false),
            direct_hit_immune: Some(|_, _, _, _| true),
            ..CombatHooks::NONE
        };
        assert!(may_sustain_by(
            [allow, allow].into_iter(),
            &state,
            content,
            DEFAULT,
            &unit
        ));
        assert!(!may_sustain_by(
            [allow, forbid, allow].into_iter(),
            &state,
            content,
            DEFAULT,
            &unit
        ));
        assert!(!direct_hit_immune_by(
            [allow, allow].into_iter(),
            &state,
            content,
            DEFAULT,
            &unit
        ));
        assert!(direct_hit_immune_by(
            [allow, forbid].into_iter(),
            &state,
            content,
            DEFAULT,
            &unit
        ));
    }

    #[test]
    fn produced_hits_sum_over_modules_in_order() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let mut table = crate::choice::Table::default();
        let (a, b, system) = (PlayerId::new("a"), PlayerId::new("b"), SystemId::new("18"));
        let site = HitSite {
            player: &a,
            opponent: &b,
            system: &system,
            round: 2,
            moment: CombatMoment::AfterRoll,
        };
        let one = CombatHooks {
            produced_hits: Some(|_, site| {
                Ok(ProducedHits {
                    any_ship: usize::try_from(site.round).unwrap_or(0),
                    ..ProducedHits::NONE
                })
            }),
            ..CombatHooks::NONE
        };
        let forced = CombatHooks {
            produced_hits: Some(|_, _| {
                Ok(ProducedHits {
                    non_fighter: 1,
                    ..ProducedHits::NONE
                })
            }),
            ..CombatHooks::NONE
        };
        let hits = crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            produced_hits_by([one, forced, CombatHooks::NONE].into_iter(), ctx, &site)
        });
        let hits = hits.unwrap();
        assert_eq!(
            hits,
            ProducedHits {
                any_ship: 2,
                producer_assigned: 0,
                non_fighter: 1
            }
        );
        assert_eq!(hits.total(), 3);
    }
}
