//! naalu: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `unit_roll_modifier` | `naalu::unit_roll_modifier` (Iconoclast against relic fragments) | Q | seam `hit_shift` |
//! | `combat.fighter_fleet_weight_halves` | `naalu::fighter_fleet_weight_halves` | Q | `combat.fighter_fleet_weight_halves` |
//! | `ground.temporary_space_commit_candidates` | `naalu::matriarch_fighters` | Q | the legacy slot (a gather of pieces) |
//! | `ground.temporary_ground_force` | `naalu::matriarch_temporary_ground_force` | Q | the legacy slot (the plan's P4 `temporary_ground_force` replacement is the window's) |
//! | `strategy.secondary_waivers` | `naalu::secondary_waivers` (Mindsieve) | Q | `strategy.secondary_waivers` |
//! | `cards.may_view_hand` | `naalu::may_view_promissory_hand` (M'aban) | blocked | needs `transactions::are_neighbours` (presence, Keleres Mecatol rule) typed: B4 |
//! | `direct.naalu_commander_has_peek` | `naalu::commander_has_peek` | Q | `direct.naalu_commander_has_peek` |
//! | `strategy.strategy_phase_ended`, `strategy.secondary_waived` | | O | `observe.rs` |
//! | `timing_abilities`, leaders | | W | B7 |

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, Planet, PromissoryNote, System, Unit, UnitBaseType};
use ti4_engine_core::hooks::ground::PieceList;
use ti4_engine_core::hooks::strategy::Waivers;
use ti4_engine_core::hooks::{CombatCtx, CombatUnit, HookTable, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::state::{CoreState, Piece};

use crate::cd_registry::has_commander_ability;
use crate::common::{flagship_has_text, is_faction, player};

const SOURCE: Source = Source::Faction(Faction::TheNaaluCollective);

/// `ti4_model::state::RULES_FACTION_BUGFIXES` (the string-side test pins the value).
pub const RULES_FACTION_BUGFIXES: u32 = 17;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, iconoclast);
    t.combat.fighter_fleet_weight_halves.push(fighter_fleet_weight_halves);
    t.ground.temporary_space_commit_candidates.push(matriarch_fighters);
    t.ground.temporary_ground_force.push(matriarch_temporary_ground_force);
    t.strategy.secondary_waivers.push(secondary_waivers);
    t.direct.naalu_commander_has_peek.push(commander_has_peek);
}

fn is_naalu(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheNaaluCollective)
}

/// Iconoclast (the Prophecy of Kings printing): +2 against an opponent on the planet who has a relic fragment.
pub fn unit_roll_modifier(s: &CoreState, _: &Kernel, unit: CombatUnit) -> i64 {
    if !s.meta().rules_at_least(RULES_FACTION_BUGFIXES) || unit.unit != Unit::Iconoclast || unit.ctx != CombatCtx::Ground {
        return 0;
    }
    let (Some(_), Some(planet)) = (unit.system, unit.planet) else { return 0 };
    let against_fragments = ti4_engine_core::query::board::planet_units(s, planet)
        .iter()
        .map(|p| p.owner())
        .filter(|owner| *owner != unit.owner)
        .any(|owner| player(s, owner).is_some_and(|p| p.cards.fragments.counts.iter().map(|n| i64::from(*n)).sum::<i64>() > 0));
    if against_fragments { 2 } else { 0 }
}

fn iconoclast(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    if !matches!(site.kind, RollKind::Combat | RollKind::Ground) {
        return Delta::None;
    }
    match unit_roll_modifier(q.s, q.k, site.unit) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Excess Hybrid Crystal Fighter IIs weigh half a ship against the fleet pool.
pub fn fighter_fleet_weight_halves(s: &CoreState, _: &Kernel, who: Seat, unit: Unit) -> bool {
    unit == Unit::HybridCrystalFighterII && is_naalu(s, who)
}

/// Whether `who` has a ship in the space area of `system` with the Matriarch's text.
fn has_matriarch(s: &CoreState, who: Seat, system: System) -> bool {
    s.board().sys(system).units.as_slice().iter().any(|p| p.owner() == who && flagship_has_text(s, who, p.kind(), Unit::Matriarch))
}

/// Matriarch lets its owner's fighters in this system join an invasion temporarily.
pub fn matriarch_fighters(s: &CoreState, _: &Kernel, invader: Seat, system: System) -> PieceList {
    let mut out = PieceList::default();
    if !has_matriarch(s, invader, system) {
        return out;
    }
    for p in s.board().sys(system).units.as_slice() {
        if p.owner() == invader && p.kind().data().base_type == UnitBaseType::Fighter {
            out.push(*p);
        }
    }
    out
}

/// A fighter counts as a ground force while its owner's Matriarch is in the system.
pub fn matriarch_temporary_ground_force(s: &CoreState, _: &Kernel, who: Seat, system: System, _planet: Planet, unit: Piece) -> bool {
    unit.kind().data().base_type == UnitBaseType::Fighter && has_matriarch(s, who, system)
}

/// The code of a Mindsieve waiver: the note and the faction whose note it is.
#[must_use]
pub fn mindsieve_code(note: PromissoryNote, faction: Faction) -> u16 {
    (u16::try_from(note.index()).unwrap_or(0) << 6) | u16::try_from(faction.index()).unwrap_or(0)
}

/// The hidden notes `who` holds: held, not faceup.
pub fn hand_notes(s: &CoreState, who: Seat) -> SmallList<ti4_engine_core::state::NoteKey, 8> {
    s.cards().promissory.iter().filter(|(key, holder)| **holder == who && !s.cards().promissory_faceup.contains_key(key)).map(|(key, _)| *key).collect()
}

/// Mindsieve: one waiver per note the follower could give (in the typed key order; the text order is the adapter's).
pub fn secondary_waivers(
    s: &CoreState,
    _: &Kernel,
    follower: Seat,
    primary: Seat,
    _: Option<ti4_engine_core::generated::StrategyCardKind>,
) -> Waivers {
    let mut out = Waivers::new();
    if follower == primary || !crate::common::holds_breakthrough(s, follower, ti4_engine_core::generated::Breakthrough::Mindsieve) {
        return out;
    }
    for key in hand_notes(s, follower).iter() {
        out.push(Waiver { module: Faction::TheNaaluCollective, code: mindsieve_code(key.note, key.faction) });
    }
    out
}

/// Whether the Naalu seat currently owns its unlocked M'aban commander (or has the ability another way).
pub fn commander_has_peek(s: &CoreState, who: Seat) -> bool {
    has_commander_ability(s, who, Leader::Maban)
}
