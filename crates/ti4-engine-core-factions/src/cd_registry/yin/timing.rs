//! yin: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Brother Milor, the Yin agent
//! ("After a player's unit is destroyed during combat: you may exhaust this card to allow that player to place 2
//! fighters in the destroyed unit's system if it was a ship, or 2 infantry on its planet if it was a ground
//! force"), the ship and the ground branch. Indoctrination is ported too (package W13), and Greyfire Mutagen (package W14), and Van Hauge (the flagship, lane M5). Not ported: the Spinner,
//! and the Ascendant (a Thunder's Edge breakthrough). The Ssruu copies of the agent are the pair rules of `yssaril::ssruu`; Ssruu copies
//! no ability of the faction, so there is no Ssruu copy of Indoctrination.
//!
//! The units are placed without a capacity check beyond the reinforcements ("place 2"; 16.3 removes the excess at the
//! end of the turn): as many as the reinforcements hold, of the form the player places (its upgrade, its faction's
//! unit, the plain one).

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, Planet, PromissoryNote, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Runner, Slot, Spec, Unknown};
use ti4_engine_core::kernel::fx_pay;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{fleet, tech};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus, NoteKey, Payload};
use ti4_engine_core::tables::Scope;

/// Brother Milor, ships (`leader:*:yinagent:SHIP_DESTROYED:after`) and ground forces (`...GROUND_FORCE_DESTROYED:after`).
pub const MILOR_SHIPS: Slot = Slot(7001);
pub const MILOR_GROUND: Slot = Slot(7002);

/// Van Hauge, the flagship (`unit:*:yin_flagship:SHIP_DESTROYED:after`): "When this ship is destroyed, destroy all ships in this system."
pub const VAN_HAUGE: Slot = Slot(7005);

/// Indoctrination (`ability:*:indoctrination:GROUND_COMBAT_STARTED:after`).
pub const INDOCTRINATION: Slot = Slot(7003);

/// Greyfire Mutagen (`promissory:*:greyfire:GROUND_COMBAT_STARTED:after`).
pub const GREYFIRE: Slot = Slot(7004);

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
        let own_units = faction.data().units;
        for base in UnitBaseType::ALL {
            // The first unit of the faction's own roster with this base type, as the content names it (`ti4_content::units::faction_unit`): a
            // newer printing in scope (the Thunder's Edge or Omega version) is the card in the box, an original out of scope is not.
            let own = own_units.iter().copied().filter_map(|unit| printing_in_scope(scope, unit)).find(|unit| unit.data().base_type == base);
            // The Corsair (Mentak, Thunder's Edge) is no unit of the faction sheet: The Table's Grace puts it over a Cruiser II
            // (`unit_form_override`); the content record of the faction lists the plain cruiser.
            roster.set(faction, base, own.filter(|unit| *unit != Unit::Corsair));
        }
    }
    roster
}

/// The printing of `unit` that is in the box: the newest one in scope that replaces it (and so on up the chain), else `unit` itself when
/// it is in scope (`ContentStore::resolve_id`).
fn printing_in_scope(scope: Scope, unit: Unit) -> Option<Unit> {
    let mut current = unit;
    while let Some(newer) = Unit::ALL.into_iter().find(|candidate| candidate.data().replaces == Some(current) && scope.unit(*candidate).is_some()) {
        current = newer;
    }
    scope.unit(current).map(|_| current)
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

/// The destroyed unit is the owner's Van Hauge. (A Nekro Alastor lent the text by the Z token of the Valefar Assimilator is a Thunder's Edge
/// breakthrough: out of the agreed content.)
// (Registered for every seat like the string: the unit in the payload says whose it is.)
fn van_hauge_when(c: &CondCx<'_>) -> bool {
    let p = &c.event.payload;
    p.unit(Key::Unit) == Some(Unit::VanHauge) && p.seat(Key::Player) == Some(c.def.owner)
}

/// Every ship still in the system goes, whoever owns it: each is removed and its destruction staged under the cause `unit_ability:yin_flagship`;
/// the emitter announces them (`announce_staged_destructions`), so a chain resolves. The enclosing combat fact is inherited from the destruction
/// that opened the window.
fn van_hauge(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::rules::combat::{assign::remove_ship, ships_of, Rd};
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let during_space_combat = eff.event.payload.flag(Key::DuringSpaceCombat) == Some(true);
    let cause = eff.codes().van_hauge;
    let (state, kernel) = eff.state_and_kernel();
    let mut owners: Vec<Seat> = Vec::new();
    for piece in &state.board().sys(system).units {
        if !owners.contains(&piece.owner()) {
            owners.push(piece.owner());
        }
    }
    let blanked = ti4_engine_core::query::reach::blanked(state);
    let mut doomed = Vec::new();
    for owner in owners {
        let ships = ships_of(Rd::new(state, kernel).with_blanked(blanked), owner, system);
        doomed.extend(ships.into_iter().map(|p| (owner, p)));
    }
    eff.inline().with_sx(|sx| {
        for (owner, piece) in doomed {
            remove_ship(sx, system, owner, piece);
            sx.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system, owner, unit: piece.kind(), reason: cause, during_space_combat });
        }
    });
    Ok(())
}

// -- Indoctrination -------------------------------------------------------------------------------------

/// Influence Indoctrination costs, and with Moyin's Ashes (the mech instead of the infantry) one more.
const INDOCTRINATION_COST: i64 = 2;
const INDOCTRINATION_MECH_COST: i64 = INDOCTRINATION_COST + 1;

/// The replacements Indoctrination can make now, in the string's order (infantry, mech): `(base type, influence cost)`, each with a unit of
/// the form the Yin places left in the reinforcements and a payment the seat can make. `Err`: a fact is missing (a payment kernel) or the seat holds
/// ocean cards. Empty unless `owner` is the Yin in the ground combat and the opponent has an infantry on the planet.
fn indoctrination_forms(s: &CoreState, f: &AdapterFacts, owner: Seat, p: &Payload) -> Result<Vec<(UnitBaseType, i64)>, ()> {
    use ti4_engine_core::query::{economy::Spend, objectives as q};
    if s.players().by_seat(owner).is_none_or(|y| y.faction != Faction::TheYinBrotherhood) {
        return Ok(Vec::new());
    }
    let (Some(attacker), Some(defender), Some(planet)) = (p.seat(Key::Attacker), p.seat(Key::Defender), p.planet(Key::Planet)) else { return Ok(Vec::new()) };
    let opponent = if owner == attacker { defender } else if owner == defender { attacker } else { return Ok(Vec::new()) };
    let has_infantry = s.board().planet(planet).units.iter().any(|u| u.owner() == opponent && u.kind().data().base_type == UnitBaseType::Infantry);
    if !has_infantry {
        return Ok(Vec::new());
    }
    let k = f.pay_kernel.ok_or(())?();
    let scope = Scope(u32::from(k.sources.0));
    let env = fx_pay::PayEnv { scope, blanked: ti4_engine_core::query::reach::blanked(s) };
    let facts = fx_pay::pay_facts(s, k, env, owner);
    if facts.terms.oceans {
        return Err(());
    }
    let mut out = Vec::new();
    for (base, cost) in [(UnitBaseType::Infantry, INDOCTRINATION_COST), (UnitBaseType::Mech, INDOCTRINATION_MECH_COST)] {
        if in_reinforcements(s, scope, owner, base).is_some() && q::first_plan(s, &facts, Spend::Influence, cost).is_some() {
            out.push((base, cost));
        }
    }
    Ok(out)
}

fn indoctrination_when(c: &CondCx<'_>) -> bool {
    match indoctrination_forms(c.state, c.facts, c.def.owner, &c.event.payload) {
        Ok(found) => !found.is_empty(),
        Err(()) => c.unknown(Unknown::MissingFact),
    }
}

fn indoctrination(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Pick, Subtype};
    let me = eff.owner();
    let Ok(forms) = indoctrination_forms(eff.read().0, eff.adapter_facts(), me, &eff.event.payload) else { return Ok(()) };
    let p = eff.event.payload.clone();
    let (Some(system), Some(planet), Some(attacker), Some(defender)) = (p.system(Key::System), p.planet(Key::Planet), p.seat(Key::Attacker), p.seat(Key::Defender)) else { return Ok(()) };
    let opponent = if me == attacker { defender } else { attacker };
    if forms.is_empty() {
        return Ok(());
    }
    let mut opts: Opts = forms.iter().map(|(base, cost)| Opt::Reaction(ROpt::Indoctrinate { base: *base, cost: i16::try_from(*cost).unwrap_or(i16::MAX), opponent })).collect();
    opts.push(Opt::Decline);
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::Indoctrination { planet }),
        ctx: Ctx {
            origin: ti4_engine_core::choice::Origin::FactionAbility(ti4_engine_core::choice::generic::Content::Other(eff.codes().indoctrination)),
            subtype: Subtype::Reaction(RSub::Indoctrination),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    let Pick::Opt(Opt::Reaction(ROpt::Indoctrinate { base, cost, .. })) = eff.ask(&choice) else { return Ok(()) };
    let before = eff.state().clone();
    if !ti4_engine_core::rules::production::pay::pay_in(eff, me, i64::from(cost), ti4_engine_core::query::economy::Spend::Influence)? {
        *eff.state() = before;
        return Ok(());
    }
    // `action_cards::replace_unit`: the first infantry of the opponent on the planet leaves, a unit of the form the Yin places takes its place.
    let scope = Scope(u32::from(eff.kernel().sources.0));
    let (form, forbidden) = {
        let (state, kernel) = eff.read();
        let form = placed_form_overridden(state, kernel, scope, me, base);
        let forbidden = form.is_some_and(|unit| kernel.hooks.economy.effect_placement_forbidden(state, kernel, me, unit));
        (form, forbidden)
    };
    let replaced = match form {
        Some(unit) if !forbidden && fleet::remaining_all(eff.state(), scope, me, unit) >= 1 => {
            let cell = eff.state().board_mut().planet_mut(planet);
            match cell.units.as_slice().iter().position(|u| u.owner() == opponent && u.kind().data().base_type == UnitBaseType::Infantry) {
                Some(at) => {
                    cell.units.remove(at);
                    fx::place_unit(eff.state(), me, system, Some(planet), unit);
                    true
                }
                None => false,
            }
        }
        _ => false,
    };
    if replaced {
        eff.state().marks_mut().tail.edit().insert(MarkKey::YinAbilityUsed(me), MarkVal::Atom(Atom::Num(1)));
    } else {
        *eff.state() = before;
    }
    Ok(())
}

// -- Greyfire Mutagen ------------------------------------------------------------------------------------

/// The Greyfire Mutagen note of the Yin Brotherhood.
const GREYFIRE_NOTE: NoteKey = NoteKey { note: PromissoryNote::GreyfireMutagen, faction: Faction::TheYinBrotherhood };

/// `greyfire_ready`: "At the start of a ground combat against 2 or more ground forces that are not controlled by the Yin player": `holder` holds
/// the note, is no Yin and fights an opponent (no Yin either) with two ground forces and an infantry on the planet, and has an infantry left in
/// its reinforcements. The opponent and the planet come from the event.
fn greyfire_target(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, holder: Seat, p: &Payload) -> Option<(Seat, Planet)> {
    let (attacker, defender, planet) = (p.seat(Key::Attacker)?, p.seat(Key::Defender)?, p.planet(Key::Planet)?);
    let opponent = if holder == attacker { defender } else if holder == defender { attacker } else { return None };
    let is_yin = |who: Seat| s.players().by_seat(who).is_some_and(|y| y.faction == Faction::TheYinBrotherhood);
    if s.cards().promissory.get(&GREYFIRE_NOTE) != Some(&holder) || is_yin(holder) || is_yin(opponent) {
        return None;
    }
    let units = &s.board().planet(planet).units;
    let ground = units.iter().filter(|u| u.owner() == opponent && fleet::is_ground_force(u.kind())).count();
    let infantry = units.iter().filter(|u| u.owner() == opponent && u.kind().data().base_type == UnitBaseType::Infantry).count();
    (ground >= 2 && infantry > 0 && in_reinforcements(s, scope(k), holder, UnitBaseType::Infantry).is_some()).then_some((opponent, planet))
}

fn greyfire_when(c: &CondCx<'_>) -> bool {
    greyfire_target(c.state, c.kernel, c.def.owner, &c.event.payload).is_some()
}

/// "Replace 1 of your opponent's infantry with 1 infantry from your reinforcements. Then, return this card to the Yin player." The replacement is
/// `action_cards::replace_unit` (the opponent's first infantry on the planet goes, a unit of the form the holder places takes its place; a placement
/// a faction effect forbids or an empty box replaces nothing, and the card stays).
fn greyfire(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let p = eff.event.payload.clone();
    // (The abilities before this one in the window may have changed the planet: the string asks `greyfire_ready` again.)
    let ready = {
        let (state, kernel) = eff.read();
        greyfire_target(state, kernel, me, &p)
    };
    let (Some((opponent, _)), Some(system)) = (ready, p.system(Key::System)) else { return Ok(()) };
    let Some(planet) = p.planet(Key::Planet) else { return Ok(()) };
    let scope = Scope(u32::from(eff.kernel().sources.0));
    let (form, forbidden) = {
        let (state, kernel) = eff.read();
        let form = placed_form_overridden(state, kernel, scope, me, UnitBaseType::Infantry);
        (form, form.is_some_and(|unit| kernel.hooks.economy.effect_placement_forbidden(state, kernel, me, unit)))
    };
    let Some(unit) = form.filter(|unit| !forbidden && fleet::remaining_all(eff.state(), scope, me, *unit) >= 1) else { return Ok(()) };
    let cell = eff.state().board_mut().planet_mut(planet);
    let Some(at) = cell.units.as_slice().iter().position(|u| u.owner() == opponent && u.kind().data().base_type == UnitBaseType::Infantry) else { return Ok(()) };
    cell.units.remove(at);
    fx::place_unit(eff.state(), me, system, Some(planet), unit);
    fx::give_back_note(eff.state(), GREYFIRE_NOTE);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(VAN_HAUGE, AbilityKind::Unit, EventKind::ShipDestroyed, Relation::After, van_hauge).when(van_hauge_when));
    t.add(Spec::new(INDOCTRINATION, AbilityKind::Ability, EventKind::GroundCombatStarted, Relation::After, indoctrination).when(indoctrination_when));
    t.add(Spec::new(GREYFIRE, AbilityKind::Promissory, EventKind::GroundCombatStarted, Relation::After, greyfire).optional().when(greyfire_when));

    t.add(Spec::new(MILOR_SHIPS, AbilityKind::Leader, EventKind::ShipDestroyed, Relation::After, |e| place(e, UnitBaseType::Fighter, false)).optional().when(ship_when));
    t.add(Spec::new(MILOR_GROUND, AbilityKind::Leader, EventKind::GroundForceDestroyed, Relation::After, |e| place(e, UnitBaseType::Infantry, true)).optional().when(ground_when));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two abilities of one table never share a slot (a second registration is a `debug_assert` of `AbilityTable::add`). Lane M5's Van Hauge and
    /// the Indoctrination of lane 13 were both `Slot(7003)` until integration 10.
    #[test]
    fn the_yin_slots_are_distinct() {
        let slots = [MILOR_SHIPS, MILOR_GROUND, INDOCTRINATION, GREYFIRE, VAN_HAUGE];
        for (i, a) in slots.iter().enumerate() {
            for b in &slots[i + 1..] {
                assert_ne!(a, b, "two yin abilities share the slot {a:?}");
            }
        }
    }

    #[test]
    fn the_whole_yin_table_registers_without_a_slot_clash() {
        let mut table = AbilityTable::default();
        register_abilities(&mut table);
    }
}
