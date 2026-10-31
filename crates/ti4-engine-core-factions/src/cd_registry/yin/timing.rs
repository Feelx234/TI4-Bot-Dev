//! yin: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Brother Milor, the Yin agent
//! ("After a player's unit is destroyed during combat: you may exhaust this card to allow that player to place 2
//! fighters in the destroyed unit's system if it was a ship, or 2 infantry on its planet if it was a ground
//! force"), the ship and the ground branch. Not ported: the Ssruu copies, Indoctrination, Greyfire, the Spinner, Van
//! Hauge, the Ascendant.
//!
//! The units are placed without a capacity check beyond the reinforcements ("place 2"; 16.3 removes the excess at the
//! end of the turn): as many as the reinforcements hold, of the form the player places (its upgrade, its faction's
//! unit, the plain one).

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{fleet, tech};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};
use ti4_engine_core::tables::Scope;

/// Brother Milor, ships (`leader:*:yinagent:SHIP_DESTROYED:after`) and ground forces (`...GROUND_FORCE_DESTROYED:after`).
pub const MILOR_SHIPS: Slot = Slot(7001);
pub const MILOR_GROUND: Slot = Slot(7002);

const AGENT: Leader = Leader::BrotherMilor;

fn ready_card(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&card) == Some(&LeaderStatus::Readied))
}

/// The faction units per faction and base type, as the source scope resolves them.
pub fn roster(scope: Scope) -> tech::UnitRoster {
    thread_local! {
        static CACHE: std::cell::Cell<Option<(u32, tech::UnitRoster)>> = const { std::cell::Cell::new(None) };
    }
    if let Some((bits, roster)) = CACHE.with(std::cell::Cell::get)
        && bits == scope.0
    {
        return roster;
    }
    let roster = build_roster(scope);
    CACHE.with(|c| c.set(Some((scope.0, roster))));
    roster
}

fn build_roster(scope: Scope) -> tech::UnitRoster {
    let mut roster = tech::UnitRoster::default();
    for faction in Faction::ALL {
        for base in UnitBaseType::ALL {
            let own = Unit::ALL.into_iter().find(|u| {
                let d = u.data();
                // The Corsair (Mentak, Thunder's Edge) is no unit of the faction sheet: The Table's Grace puts it over a Cruiser II
                // (`unit_form_override`); the content record of the faction lists the plain cruiser (`ti4_content::units::faction_unit`).
                *u != Unit::Corsair && d.base_type == base && d.upgrades_from.is_none() && d.faction.is_some_and(|tag| tech::tag_is(tag, faction)) && scope.unit(*u).is_some()
            });
            roster.set(faction, base, own);
        }
    }
    roster
}

/// `placed_unit_id` of a seat for a base type: the upgrade it owns, else its faction's own unit, else the plain one.
pub fn placed_form(s: &CoreState, scope: Scope, who: Seat, base: UnitBaseType) -> Option<Unit> {
    tech::unit_form(s, scope, &roster(scope), who, base)
}

/// [`placed_form`] with the faction override of the hook table (`action_cards::placed_unit_id`: Mentak's Corsair).
pub fn placed_form_overridden(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, scope: Scope, who: Seat, base: UnitBaseType) -> Option<Unit> {
    ti4_engine_core::kernel::fx_strategy::placed_unit(s, k, scope, &roster(scope), who, base)
}

/// `in_reinforcements`: the form the player places, and one of it left in the box.
fn in_reinforcements(s: &CoreState, scope: Scope, who: Seat, base: UnitBaseType) -> Option<Unit> {
    let unit = placed_form(s, scope, who, base)?;
    (fleet::remaining_all(s, scope, who, unit) > 0).then_some(unit)
}

fn scope(k: &ti4_engine_core::kernel::Kernel) -> Scope {
    crate::common::scope(k)
}

fn ship_when(c: &CondCx<'_>) -> bool {
    ship_ok(c, AGENT)
}

/// The condition of the ship branch with `card` (Brother Milor, or Ssruu copying it) as the exhausted card.
pub(crate) fn ship_ok(c: &CondCx<'_>, card: Leader) -> bool {
    let p = &c.event.payload;
    p.flag(Key::DuringSpaceCombat) == Some(true)
        && ready_card(c.state, c.def.owner, card)
        && p.seat(Key::Player).is_some_and(|who| in_reinforcements(c.state, scope(c.kernel), who, UnitBaseType::Fighter).is_some())
}

fn ground_when(c: &CondCx<'_>) -> bool {
    ground_ok(c, AGENT)
}

/// The condition of the ground branch with `card` as the exhausted card.
pub(crate) fn ground_ok(c: &CondCx<'_>, card: Leader) -> bool {
    let p = &c.event.payload;
    let cause = p.code(Key::Cause);
    (cause == Some(c.codes.ground_combat) || cause == Some(c.codes.harrow))
        && p.unit(Key::Unit).is_some()
        && ready_card(c.state, c.def.owner, card)
        && p.seat(Key::Player).is_some_and(|who| in_reinforcements(c.state, scope(c.kernel), who, UnitBaseType::Infantry).is_some())
}

/// `place_units_counted`: up to `count` units of the form, as many as the box holds, appended where they go.
fn place(eff: &mut Eff<'_, '_>, base: UnitBaseType, on_planet: bool) -> Result<(), TimingError> {
    place_by(eff, base, on_planet, AGENT)
}

/// [`place`] with `card` (Brother Milor, or Ssruu copying it) exhausted for the use.
pub(crate) fn place_by(eff: &mut Eff<'_, '_>, base: UnitBaseType, on_planet: bool, card: Leader) -> Result<(), TimingError> {
    let owner = eff.owner();
    let p = &eff.event.payload;
    let (Some(system), Some(who)) = (p.system(Key::System), p.seat(Key::Player)) else { return Ok(()) };
    let planet = if on_planet { p.planet(Key::Planet) } else { None };
    if on_planet && planet.is_none() {
        return Ok(());
    }
    if !ready_card(eff.state(), owner, card) {
        return Ok(());
    }
    let scope = scope(eff.kernel());
    let Some(unit) = in_reinforcements(eff.state(), scope, who, base) else { return Ok(()) };
    // `leaders::exhaust`: the status, and the staged announcement a watching Nomad is told by.
    let (state, kernel) = eff.state_and_kernel();
    ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, owner, card);
    let count = fleet::remaining_all(eff.state(), scope, who, unit).min(2);
    for _ in 0..count {
        fx::place_unit(eff.state(), who, system, planet, unit);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MILOR_SHIPS, AbilityKind::Leader, EventKind::ShipDestroyed, Relation::After, |e| place(e, UnitBaseType::Fighter, false)).optional().when(ship_when));
    t.add(Spec::new(MILOR_GROUND, AbilityKind::Leader, EventKind::GroundForceDestroyed, Relation::After, |e| place(e, UnitBaseType::Infantry, true)).optional().when(ground_when));
}
