//! crimson: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `combat.may_sustain`, `ground.may_sustain` | `crimson::may_sustain` | Q | seam `may_sustain` (one rule for both contexts) |
//! | `crimson_abilities_lost` | `crimson::abilities_lost` | Q | direct (`silenced`'s bridge reads it) |
//! | `crimson_active_breaches` | `crimson::active_breaches` | Q | direct |
//! | `crimson_deploy_planets` | `crimson::deploy_planets` | Q | direct (`deploy_planets`' bridge reads it) |
//! | `crimson_can_deploy` | `crimson::can_deploy` | W | needs `invasion::landable_planets` (B2): see `offer.rs` |
//! | `crimson_cards_offer_card_spot` | `crimson_cards::offer_card_spot` | W | a card spot is not a `System`: needs B3's spot type |
//! | movement slots | `crimson::{usable_wormhole_kinds, linked_systems, move_bonus}`, `crimson_cards::severed_systems` | Q | `movement.rs` (A2) |

use ti4_engine_core::generated::{Faction, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::direct::{Planets, Systems};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::kernel::seam::{CombatUnit, Q, Source, Verdict};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::fleet;
use ti4_engine_core::state::CoreState;

use crate::common::{is_faction, mark, scope};

const SOURCE: Source = Source::Faction(Faction::TheCrimsonRebellion);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.may_sustain.push(SOURCE, may_sustain);
    let d = &mut t.direct;
    d.crimson_abilities_lost.push(abilities_lost);
    d.crimson_active_breaches.push(active_breaches);
    d.crimson_deploy_planets.push(deploy_planets);
}

fn is_active(s: &CoreState, system: System) -> bool {
    s.explore().breach.contains(system) && mark(s, MarkKey::CrimsonBreachActive(system)).is_some()
}

/// Every system holding an active breach, in id order.
fn active_breaches(s: &CoreState) -> Systems {
    s.explore().breach.iter().filter(|b| mark(s, MarkKey::CrimsonBreachActive(*b)).is_some()).collect()
}

/// Whether `owner`'s units in `system` have lost every unit ability to a Quietus: the system holds an active breach
/// and another player's Quietus (or a Nekro flagship carrying its text) stands in a system holding one.
fn abilities_lost(s: &CoreState, owner: Seat, system: System) -> bool {
    if s.explore().breach.is_empty() || !is_active(s, system) {
        return false;
    }
    let active = active_breaches(s);
    s.players().iter().any(|p| {
        p.seat != owner
            && active.iter().any(|at| {
                s.board().sys(*at).units.as_slice().iter().any(|piece| {
                    piece.owner() == p.seat && crate::common::flagship_has_text(s, p.seat, piece.kind(), Unit::Quietus)
                })
            })
    })
}

/// SUSTAIN DAMAGE is a unit ability: a unit that has lost them cannot use it (space and ground).
fn may_sustain(q: Q<'_>, unit: &CombatUnit) -> Verdict {
    if unit.system.is_some_and(|system| abilities_lost(q.s, unit.owner, system)) {
        Verdict::Forbid
    } else {
        Verdict::Abstain
    }
}

/// The ships of the hero card of `who` whose base type is `base`.
fn on_card(s: &CoreState, who: Seat, base: UnitBaseType) -> i64 {
    let Some(MarkVal::List(items)) = mark(s, MarkKey::CrimsonCard(who)) else {
        return 0;
    };
    items
        .iter()
        .filter(|v| matches!(v, MarkVal::Atom(Atom::Unit(u)) if u.data().base_type == base))
        .count() as i64
}

/// `supply::allowed(.., Revenant, 1) == 0`.
fn no_mech_left(s: &CoreState, k: &Kernel, who: Seat) -> bool {
    let mech = Unit::Revenant;
    if k.hooks.economy.effect_placement_forbidden(s, k, who, mech) {
        return true;
    }
    fleet::remaining(s, scope(k), who, mech, on_card(s, who, mech.data().base_type)) <= 0
}

/// The planets of `system` where `invader` may DEPLOY a Revenant while committing ground forces.
fn deploy_planets(s: &CoreState, k: &Kernel, invader: Seat, system: System, landable: &[Planet]) -> Planets {
    let deployed = mark(s, MarkKey::CrimsonDeployed(invader, DecimalTextOrd(s.meta().activation_seq))).is_some();
    if !is_faction(s, invader, Faction::TheCrimsonRebellion) || !is_active(s, system) || deployed || no_mech_left(s, k, invader) {
        return Planets::default();
    }
    Planets::from_slice(landable)
}
