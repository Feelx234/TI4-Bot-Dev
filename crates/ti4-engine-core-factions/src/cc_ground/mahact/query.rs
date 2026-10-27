//! mahact: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `mahact_fleet_pool_owners` | `mahact::fleet_pool_owners` | Q | direct (concat) |
//! | `mahact_foreign_tokens` | `mahact::foreign_tokens` | Q | direct |
//! | `mahact_imperia_grants` | `mahact::imperia_grants` | Q | direct |
//! | `mahact_is_mahact` | `mahact::is_mahact` | Q | direct |
//! | `mahact_purges_alliance` | `mahact::purges_alliance` | Q | direct |
//! | `mahact_units_flagship_roll_bonus` | `mahact_units::flagship_roll_bonus` | Q | seam `hit_shift` (native; the direct slot stays empty so the legacy bridge adds nothing) |
//! | `mahact_units_ship_movement_barred` | `mahact_units::ship_movement_barred` | Q | direct |
//! | `strategy.secondary_waivers` | `mahact::secondary_waivers` | Q | `strategy.secondary_waivers` (code = the system's index) |
//! | `strategy.secondary_waived` | `mahact::secondary_waived` | W | `offer.rs`: needs the kernel's leader exhaust (it tells the Nomad's Suite) |
//! | `mahact_units_offer_starlancer` | `mahact_units::offer_starlancer` | W | `offer.rs` |
//! | `commander_unlocked`, component actions, `leader_action`, `use_leader_timed`, timing abilities | | W | B7 |
//!
//! The string module has no `control_gained` hook: tier O is empty.

use ti4_engine_core::generated::{Faction, Leader, Unit};
use ti4_engine_core::hooks::strategy::Waivers;
use ti4_engine_core::hooks::{HookTable, Waiver};
use ti4_engine_core::hooks::direct::Seats;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{MarkKey, MarkVal};
use ti4_engine_core::hooks::CombatCtx;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::state::{CoreState, LeaderStatus};

use crate::common::{flagship_has_text, is_faction, mark, player, seat as seat_of};

const SOURCE: Source = Source::Faction(Faction::TheMahactGeneSorcerers);

/// The roll bonus of Arvicon Rex.
const FLAGSHIP_BONUS: i64 = 2;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, arvicon_rex);
    let d = &mut t.direct;
    d.mahact_fleet_pool_owners.push(fleet_pool_owners);
    d.mahact_foreign_tokens.push(foreign_tokens);
    d.mahact_imperia_grants.push(imperia_grants);
    d.mahact_is_mahact.push(is_mahact);
    d.mahact_purges_alliance.push(purges_alliance);
    d.mahact_units_ship_movement_barred.push(ship_movement_barred);
    t.strategy.secondary_waivers.push(secondary_waivers);
}

/// Whether `who` plays the Mahact.
pub fn is_mahact(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheMahactGeneSorcerers)
}

/// Whether a faction purges its Alliance note during setup (Hubris).
pub fn purges_alliance(faction: Faction) -> bool {
    faction == Faction::TheMahactGeneSorcerers
}

/// The owners of the other players' command tokens in `mahact`'s fleet pool, one entry per token, sorted.
pub fn fleet_pool_owners(s: &CoreState, mahact: Seat) -> Seats {
    match mark(s, MarkKey::MahactFleet(mahact)) {
        Some(MarkVal::List(items)) => items.iter().filter_map(seat_of).collect(),
        _ => Seats::default(),
    }
}

/// How many foreign command tokens `who`'s fleet pool holds (0 unless they are the Mahact).
pub fn foreign_tokens(s: &CoreState, who: Seat) -> i32 {
    if !is_mahact(s, who) {
        return 0;
    }
    i32::try_from(fleet_pool_owners(s, who).len()).unwrap_or(i32::MAX)
}

/// Imperia: `who`, a Mahact, can use `commander`'s ability because that commander's owner has a command token in their
/// fleet pool and the commander is unlocked.
pub fn imperia_grants(s: &CoreState, who: Seat, commander: Leader) -> bool {
    if !is_mahact(s, who) {
        return false;
    }
    fleet_pool_owners(s, who)
        .iter()
        .any(|owner| *owner != who && player(s, *owner).is_some_and(|p| p.leaders.status.get(&commander) == Some(&LeaderStatus::Unlocked)))
}

/// Arvicon Rex on a space combat roll (a native provider: the direct slot stays empty so the legacy bridge adds nothing).
fn arvicon_rex(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    let u = &site.unit;
    if site.kind != RollKind::Combat || u.ctx != CombatCtx::Space {
        return Delta::None;
    }
    match flagship_roll_bonus(q.s, q.k, u.owner, u.unit) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Arvicon Rex: +2 for the flagship when every opponent in the combat in the active system has no command token in the
/// flagship owner's fleet pool (a neutral opponent has none).
pub fn flagship_roll_bonus(s: &CoreState, k: &Kernel, who: Seat, unit: Unit) -> i64 {
    if !flagship_has_text(s, who, unit, Unit::ArviconRex) {
        return 0;
    }
    let Some(system) = s.meta().active_system else { return 0 };
    let pool = fleet_pool_owners(s, who);
    let mut opponents = super::super::sides::combatants(s, k, system);
    opponents.remove(who);
    if opponents.is_empty() || opponents.iter().any(|side| pool.contains(&side)) {
        return 0;
    }
    FLAGSHIP_BONUS
}

/// Whether the hero's combat is being fought: neither player can retreat or resolve an ability that would move ships.
pub fn ship_movement_barred(s: &CoreState) -> bool {
    mark(s, MarkKey::MahactBenediction()).is_some()
}

/// Jae Mir Kan: one waiver per system holding a command token of the active player, offered to a Mahact with the
/// agent readied. The waiver's code is the system's index.
pub fn secondary_waivers(
    s: &CoreState,
    _: &Kernel,
    follower: Seat,
    primary: Seat,
    _: Option<ti4_engine_core::generated::StrategyCardKind>,
) -> Waivers {
    let mut out = Waivers::new();
    if follower == primary
        || !is_mahact(s, follower)
        || player(s, follower).is_none_or(|p| p.leaders.status.get(&Leader::JaeMirKan) != Some(&LeaderStatus::Readied))
    {
        return out;
    }
    for (system, cell) in s.board().systems() {
        if cell.cmd.contains(primary) {
            out.push(Waiver { module: Faction::TheMahactGeneSorcerers, code: system.index() as u16 });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cc_ground::testkit::table;
    use ti4_engine_core::keys::Atom;
    use ti4_engine_core::state::Piece;

    const MA: Faction = Faction::TheMahactGeneSorcerers;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn pool(s: &mut CoreState, owners: &[u8]) {
        let items = owners.iter().map(|o| MarkVal::Atom(Atom::Seat(Seat(*o)))).collect();
        s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(items));
    }

    #[test]
    fn the_fleet_pool_counts_foreign_tokens_and_imperia_grants_unlocked_commanders() {
        let (mut s, _) = table(&[MA, SOL]);
        assert_eq!(foreign_tokens(&s, Seat(0)), 0);
        pool(&mut s, &[1]);
        assert_eq!(foreign_tokens(&s, Seat(0)), 1);
        assert_eq!(foreign_tokens(&s, Seat(1)), 0, "only the Mahact");
        assert_eq!(fleet_pool_owners(&s, Seat(0)).as_slice(), &[Seat(1)]);
        assert!(!imperia_grants(&s, Seat(0), Leader::ArozHollow), "locked");
        s.players_mut().edit(1).leaders.status.insert(Leader::ArozHollow, LeaderStatus::Unlocked);
        assert!(imperia_grants(&s, Seat(0), Leader::ArozHollow));
        assert!(!imperia_grants(&s, Seat(1), Leader::ArozHollow), "not the Mahact");
    }

    #[test]
    fn arvicon_rex_needs_every_opponent_out_of_the_pool() {
        let (mut s, _) = table(&[MA, SOL]);
        let system = ti4_engine_core::ids::system_at(30);
        s.meta_mut().active_system = Some(system);
        let k = Kernel::plain();
        let scope_all = Kernel { sources: ti4_engine_core::kernel::Sources(u8::MAX), ..Kernel::plain() };
        let _ = k;
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::ArviconRex, Seat(0)));
        assert_eq!(flagship_roll_bonus(&s, &scope_all, Seat(0), Unit::ArviconRex), 0, "no opponent in the system");
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        assert_eq!(flagship_roll_bonus(&s, &scope_all, Seat(0), Unit::ArviconRex), 2);
        assert_eq!(flagship_roll_bonus(&s, &scope_all, Seat(0), Unit::CruiserI), 0, "not the flagship");
        pool(&mut s, &[1]);
        assert_eq!(flagship_roll_bonus(&s, &scope_all, Seat(0), Unit::ArviconRex), 0, "their token is in the pool");
    }

    #[test]
    fn the_agent_waives_one_token_per_system() {
        let (mut s, k) = table(&[MA, SOL]);
        let system = ti4_engine_core::ids::system_at(30);
        s.board_mut().sys_mut(system).cmd.insert(Seat(1));
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(1), None).is_empty(), "agent not readied");
        s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Readied);
        let offered = secondary_waivers(&s, &k, Seat(0), Seat(1), None);
        assert_eq!(offered.as_slice(), &[Waiver { module: MA, code: system.index() as u16 }]);
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(0), None).is_empty(), "not on your own card");
        assert!(!ship_movement_barred(&s));
    }
}
