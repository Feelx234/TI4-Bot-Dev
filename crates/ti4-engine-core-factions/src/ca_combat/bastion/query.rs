//! bastion: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `unit_roll_modifier` | `bastion_units::unit_roll_modifier` | Q | seam `hit_shift` |
//! | `bastion_extra_die` | `bastion::extra_die` | Q | direct (the dice stack's bridge reads it) |
//! | `bastion_units_resource_bonus` | `bastion_units::resource_bonus` | Q | direct (`planet_spend`'s bridge) |
//! | `bastion_units_cancel_bombardment_hits` | `bastion_units::cancel_bombardment_hits` | Q | direct |
//! | `bastion_units_cancel_in_groups` | `bastion_units::cancel_in_groups` | Q | direct |
//! | `bastion_units_watches_ground_round` | `bastion_units::watches_ground_round` | Q | direct |
//! | `bastion_units_non_home_systems_with_planets` | `bastion_units::non_home_systems_with_planets` | Q | direct |
//!
//! Not Q: see `observe.rs` (ship lost / retreat bookkeeping) and `offer.rs` (`adjust_ship_spots`,
//! `ship_placed`, `timing_abilities`, `commander_unlocked`). `bastion_commander_blocks_sabotage` has no string
//! function: `reactions.rs` tests the leader text inline.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, PlanetTrait, System, Technology, Unit};
use ti4_engine_core::hooks::{CombatCtx, HookTable};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollSite, Source};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, Piece};

use crate::common::{flagship_has_text, has_tech_text};

const SOURCE: Source = Source::Faction(Faction::LastBastion);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, egeiro);
    let d = &mut t.direct;
    d.bastion_extra_die.push(extra_die);
    d.bastion_units_resource_bonus.push(resource_bonus);
    d.bastion_units_cancel_bombardment_hits.push(cancel_bombardment_hits);
    d.bastion_units_cancel_in_groups.push(cancel_in_groups);
    d.bastion_units_watches_ground_round.push(watches_ground_round);
    d.bastion_units_non_home_systems_with_planets.push(non_home_systems_with_planets);
}

/// `bastion_units::faction_in_game`.
fn faction_in_game(s: &CoreState) -> bool {
    s.players().iter().any(|p| p.faction == Faction::LastBastion)
}

/// The Egeiro: +1 to each combat roll for each non-home system with a planet its owner controls.
fn egeiro(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    let u = &site.unit;
    if site.kind != ti4_engine_core::kernel::seam::RollKind::Combat
        || u.ctx != CombatCtx::Space
        || !faction_in_game(q.s)
        || !flagship_has_text(q.s, u.owner, u.unit, Unit::TheEgeiro)
    {
        return Delta::None;
    }
    Delta::Add(i64::try_from(non_home_systems_with_planets(q.s, q.k, u.owner)).unwrap_or(i64::MAX))
}

/// "1 additional die" of a galvanized unit.
fn extra_die(_: &CoreState, piece: Piece) -> usize {
    usize::from(piece.galvanized())
}

/// Non-home systems that contain a planet `who` controls; a space station is not a planet.
fn non_home_systems_with_planets(s: &CoreState, _: &Kernel, who: Seat) -> usize {
    let homes: Vec<System> = s.players().iter().filter_map(|p| p.home_system).collect();
    let mut systems: SmallList<System, 16> = SmallList::default();
    for (system, planet) in board::controlled_planets(s, who).iter() {
        if homes.contains(system)
            || planet.data().traits.contains(&PlanetTrait::Spacestation)
            || systems.as_slice().contains(system)
        {
            continue;
        }
        systems.push(*system);
    }
    systems.len()
}

/// What the Helios docks add to a planet's resource value: +1 for V1, +2 for V2.
fn resource_bonus(s: &CoreState, planet: Planet) -> i64 {
    if !faction_in_game(s) {
        return 0;
    }
    board::planet_units(s, planet)
        .iter()
        .map(|p| match p.kind() {
            Unit::BastionSpacedock => 1,
            Unit::BastionSpacedock2 => 2,
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Galvanized units of `owner` standing on `planet` of `system`.
fn galvanized_present(s: &CoreState, owner: Seat, system: System, planet: Planet) -> usize {
    if board::home_system(s, planet) != Some(system) {
        return 0;
    }
    board::planet_units(s, planet).iter().filter(|p| p.owner() == owner && p.galvanized()).count()
}

/// Hits left after Proxima Targeting VI's cancellation.
fn cancel_bombardment_hits(s: &CoreState, victim: Seat, system: System, planet: Planet, hits: usize) -> usize {
    if hits == 0 || !has_tech_text(s, victim, Technology::ProximaTargetingVI) {
        return hits;
    }
    hits.saturating_sub(galvanized_present(s, victim, system, planet))
}

/// [`cancel_bombardment_hits`] over the per-unit hit groups of one planet's bombardment.
fn cancel_in_groups(s: &CoreState, victim: Seat, system: System, planet: Planet, groups: &mut SmallList<usize, 8>) {
    if groups.is_empty() || !has_tech_text(s, victim, Technology::ProximaTargetingVI) {
        return;
    }
    let mut budget = galvanized_present(s, victim, system, planet);
    for group in groups.as_mut_slice() {
        let cancelled = (*group).min(budget);
        *group -= cancelled;
        budget -= cancelled;
    }
    groups.retain(|g| *g > 0);
}

/// Whether a seated player of this combat holds Proxima Targeting VI.
fn watches_ground_round(s: &CoreState, sides: &[Seat]) -> bool {
    sides.iter().any(|side| has_tech_text(s, *side, Technology::ProximaTargetingVI))
}
