//! mentak: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `combat.may_sustain` | `mentak::flagship_allows_sustain` | Q | seam `may_sustain` (Fourth Moon, space) |
//! | `ground.may_sustain` | `mentak::mech_allows_sustain` | Q | seam `may_sustain` (Moll Terminus, ground) |
//! | `economy.trade_good_worth` | `mentak::trade_good_worth` | Q | `economy.trade_good_worth` (a fold) |
//! | `strategy.unit_form_override` | `mentak::unit_form_override` | Q | `strategy.unit_form_override` (P4 first) |
//! | `movement.may_move_through_ships` | `mentak::may_move_through_ships` | Q | `movement.rs` (A2, done) |
//! | `produced_hits`, `commander_unlocked`, component actions, timing abilities, leaders | | W | `offer.rs` / B7 |

use ti4_engine_core::generated::{Breakthrough, Faction, Technology, Unit, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::CombatCtx;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{CombatUnit, Q, Source, Verdict};
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{flagship_has_text, has_tech_text, holds_breakthrough, is_faction};

const SOURCE: Source = Source::Faction(Faction::TheMentakCoalition);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.may_sustain.push(SOURCE, may_sustain);
    t.economy.trade_good_worth.push(trade_good_worth);
    t.strategy.unit_form_override.push(unit_form_override);
}

/// Another player's `unit` of `except`'s opponents stands in `system` (`foreign_ship_in`, by flagship text).
fn foreign_ship_in(s: &CoreState, system: ti4_engine_core::generated::System, flagship: Unit, except: Seat) -> bool {
    board::space_units(s, system).iter().any(|u| u.owner() != except && flagship_has_text(s, u.owner(), u.kind(), flagship))
}

/// Fourth Moon: "Other player's ships in this system cannot use SUSTAIN DAMAGE." Moll Terminus: "Other player's ground
/// forces on this planet cannot use SUSTAIN DAMAGE."
fn may_sustain(q: Q<'_>, unit: &CombatUnit) -> Verdict {
    let forbidden = match unit.ctx {
        CombatCtx::Space => unit.system.is_some_and(|system| foreign_ship_in(q.s, system, Unit::FourthMoon, unit.owner)),
        CombatCtx::Ground => match (unit.system, unit.planet) {
            (Some(_), Some(planet)) => board::planet_units(q.s, planet).iter().any(|u| u.kind() == Unit::MollTerminus && u.owner() != unit.owner),
            _ => false,
        },
    };
    if forbidden { Verdict::Forbid } else { Verdict::Abstain }
}

/// Mirror Computing: each trade good is worth 2 instead of 1 (`max`, so another effect's raise stays).
fn trade_good_worth(s: &CoreState, who: Seat, worth: i64) -> i64 {
    if has_tech_text(s, who, Technology::MirrorComputing) { worth.max(2) } else { worth }
}

/// The Table's Grace: the Corsair stands in for Cruiser II for the holder.
pub fn unit_form_override(s: &CoreState, _: &Kernel, who: Seat, base: UnitBaseType, chosen: Unit) -> Option<Unit> {
    (base == UnitBaseType::Cruiser
        && chosen == Unit::CruiserII
        && is_faction(s, who, Faction::TheMentakCoalition)
        && holds_breakthrough(s, who, Breakthrough::TheTablesGrace))
    .then_some(Unit::Corsair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cc_ground::testkit::table;
    use ti4_engine_core::state::Piece;

    const M: Faction = Faction::TheMentakCoalition;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn site(owner: Seat, unit: Unit, ctx: CombatCtx, planet: Option<ti4_engine_core::generated::Planet>) -> CombatUnit {
        let system = ti4_engine_core::ids::system_at(30);
        CombatUnit { owner, system: Some(system), planet, unit, ctx }
    }

    #[test]
    fn the_fourth_moon_forbids_other_players_ships_to_sustain() {
        let (mut s, k) = table(&[M, SOL]);
        let system = ti4_engine_core::ids::system_at(30);
        let kernel = {
            let mut t = HookTable::new();
            register(&mut t);
            Kernel { hooks: t, ..k }
        };
        let q = Q::new(&s, &kernel);
        let theirs = site(Seat(1), Unit::CruiserI, CombatCtx::Space, None);
        assert_eq!(may_sustain(q, &theirs), Verdict::Abstain, "no flagship");
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::FourthMoon, Seat(0)));
        let q = Q::new(&s, &kernel);
        assert_eq!(may_sustain(q, &theirs), Verdict::Forbid);
        assert_eq!(may_sustain(q, &site(Seat(0), Unit::FourthMoon, CombatCtx::Space, None)), Verdict::Abstain, "its own ships may");
    }

    #[test]
    fn mirror_computing_and_the_corsair() {
        let (mut s, k) = table(&[M, SOL]);
        assert_eq!(trade_good_worth(&s, Seat(0), 1), 1);
        s.players_mut().edit(0).tech.owned.insert(Technology::MirrorComputing);
        assert_eq!(trade_good_worth(&s, Seat(0), 1), 2);
        assert_eq!(trade_good_worth(&s, Seat(0), 3), 3, "max, not a sum");
        assert_eq!(unit_form_override(&s, &k, Seat(0), UnitBaseType::Cruiser, Unit::CruiserII), None);
        s.players_mut().edit(0).cards.breakthrough = Some(Breakthrough::TheTablesGrace);
        assert_eq!(unit_form_override(&s, &k, Seat(0), UnitBaseType::Cruiser, Unit::CruiserII), Some(Unit::Corsair));
        assert_eq!(unit_form_override(&s, &k, Seat(0), UnitBaseType::Cruiser, Unit::CruiserI), None);
        assert_eq!(unit_form_override(&s, &k, Seat(1), UnitBaseType::Cruiser, Unit::CruiserII), None);
    }
}
