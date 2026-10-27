//! yin: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `strategy.extra_prerequisite_colours` | `yin::extra_prerequisite_colours` (Brother Omar) | Q | `strategy.extra_prerequisite_colours` |
//! | `strategy.research_waiver_offer` | `yin::research_waiver_offer` (Brother Omar) | Q | `strategy.research_waiver_offer` (a `first`; payment codes below) |
//! | `general.commander_unlocked` | `yin::commander_unlocked` | Q | `general.commander_unlocked` |
//! | `strategy.research_waiver_paid` | `yin::research_waiver_paid` | W | `offer.rs` (it removes the infantry) |
//! | `combat.produced_hits` | `yin::produced_hits` | W | `offer.rs` (asks) |
//! | `leader_action`, `use_leader`, `use_leader_timed`, `timing_abilities` | | W | B7 |
//!
//! Payment codes of the research waiver: a planet `p` is `p.index() + 1`; the space area of system `y` is
//! `SPACE | y.index()`. The text ids (`ground|<system>|<planet>`, `space|<system>`) are the adapter's.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, System, TechType, Technology, UnitBaseType};
use ti4_engine_core::hooks::strategy::Colours;
use ti4_engine_core::hooks::{ExtraColour, HookTable, ResearchWaiver, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::cd_registry::has_commander_ability;
use crate::common::mark;

/// The code of the "return 1 infantry" waiver.
pub const YIN_INFANTRY: u16 = 0;
/// Set in the code of a payment from a space area.
pub const SPACE: u16 = 0x8000;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.strategy.extra_prerequisite_colours.push(extra_prerequisite_colours);
    t.strategy.research_waiver_offer.push(research_waiver_offer);
}

/// Brother Omar, unlock: "Use one of your faction abilities." Recorded by Indoctrination and Devotion when they resolve.
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::BrotherOmar).then(|| mark(s, MarkKey::YinAbilityUsed(who)).is_some())
}

/// Whether `who` currently has Brother Omar's commander ability.
pub fn commander_active(s: &CoreState, who: Seat) -> bool {
    has_commander_ability(s, who, Leader::BrotherOmar)
}

/// Brother Omar: "This card satisfies a green technology prerequisite."
pub fn extra_prerequisite_colours(s: &CoreState, _: &Kernel, who: Seat) -> Colours {
    let mut out = Colours::new();
    if commander_active(s, who) {
        out.push(ExtraColour { colour: TechType::Biotic, count: 1 });
    }
    out
}

/// Every location where `who` may return an infantry to reinforcements: `(code, system, planet)`.
pub fn infantry_payments(s: &CoreState, who: Seat) -> SmallList<(u16, System, Option<ti4_engine_core::generated::Planet>), 8> {
    let is_mine = |p: &ti4_engine_core::state::Piece| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry;
    let mut out = SmallList::default();
    for (system, cell) in s.board().systems() {
        for planet in board::planets_of(s, system) {
            if board::planet_units(s, planet).iter().any(is_mine) {
                out.push((u16::try_from(planet.index() + 1).unwrap_or(0), system, Some(planet)));
            }
        }
        if cell.units.as_slice().iter().any(is_mine) {
            out.push((SPACE | u16::try_from(system.index()).unwrap_or(0), system, None));
        }
    }
    out
}

/// Brother Omar: "When you research a tech owned by another player, you may return 1 of your infantry to reinforcements
/// to ignore its prerequisites."
pub fn research_waiver_offer(s: &CoreState, _: &Kernel, who: Seat, tech: Technology) -> Option<ResearchWaiver> {
    let owned_elsewhere = s.players().iter().any(|p| p.seat != who && p.tech.owned.contains(tech));
    if !(commander_active(s, who) && owned_elsewhere) {
        return None;
    }
    let payments = infantry_payments(s, who);
    if payments.is_empty() {
        return None;
    }
    Some(ResearchWaiver {
        waiver: Waiver { module: Faction::TheYinBrotherhood, code: YIN_INFANTRY },
        payments: payments.iter().map(|(code, _, _)| Waiver { module: Faction::TheYinBrotherhood, code: *code }).collect(),
    })
}

