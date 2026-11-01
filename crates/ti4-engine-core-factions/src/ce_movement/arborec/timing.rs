//! arborec: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Letani Warrior II, the
//! roll ("After this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card.").
//! The units on the card are a count in the owner's `ArborecLw2` mark; they come back at the start of the next turn
//! (a placement, the string's). Not ported: Mitosis, Bioplasmosis, the flagship, the commander, Stymie, the return.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::tech;
use ti4_engine_core::state::{CoreState, EventKind, Key, RollPurpose};

/// Letani Warrior II (`unit:arborec:arborec_infantry2:GROUND_FORCE_DESTROYED:after`).
pub const WARRIOR_ROLLS: Slot = Slot(6001);

/// The die result that puts the unit on the card.
const THRESHOLD: u8 = 6;
/// `Dice::new()`: ten sides.
const SIDES: u32 = 10;

fn on_card(s: &CoreState, who: Seat) -> u32 {
    match s.marks().tail.get(&MarkKey::ArborecLw2(who)) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && c.event.payload.unit(Key::Unit) == Some(Unit::LetaniWarriorII) && tech::owns(c.state, me, Technology::LetaniWarriorII)
}

fn rolls(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let faces = eff.roll(me, 1, Some(THRESHOLD), RollPurpose::Ability(WARRIOR_ROLLS.0), SIDES);
    if faces.first().is_some_and(|f| *f >= THRESHOLD) {
        let now = on_card(eff.state(), me) + 1;
        eff.state().marks_mut().tail.edit().insert(MarkKey::ArborecLw2(me), MarkVal::Atom(Atom::Num(now)));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(
        Spec::new(WARRIOR_ROLLS, AbilityKind::Unit, EventKind::GroundForceDestroyed, Relation::After, rolls)
            .applies(Applies::Faction(Faction::TheArborec))
            .when(condition),
    );
}

// -- Duha Menaimon and Dirzuga Rophal (lane M6d) -----------------------------------------------------------------------------------

use ti4_engine_core::driver::windows::production::{can_produce_by_ability, produce_in};
use ti4_engine_core::generated::{Leader, System};
use ti4_engine_core::query::{leaders, reach};

/// Duha Menaimon (`unit:*:arborec_flagship:SYSTEM_ACTIVATED:after`).
pub const FLAGSHIP: Slot = Slot(50_010);
/// Dirzuga Rophal (`leader:*:arboreccommander:SYSTEM_ACTIVATED:after`).
pub const COMMANDER: Slot = Slot(50_011);

/// "Up to 5 units" (`FLAGSHIP_LIMIT`).
const FLAGSHIP_LIMIT: i32 = 5;

fn activated(c: &CondCx<'_>) -> Option<(Seat, System)> {
    let p = &c.event.payload;
    Some((p.seat(Key::Player)?, p.system(Key::System)?))
}

/// The seat has its flagship, or a Nekro flagship carrying the text, in the space area of `system`.
fn flagship_in(s: &CoreState, who: Seat, system: System) -> bool {
    ti4_engine_core::query::board::space_units(s, system).iter().any(|p| p.owner() == who && crate::common::flagship_has_text(s, who, p.kind(), Unit::DuhaMenaimon))
}

fn flagship_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    activated(c).is_some_and(|(activator, system)| activator == me && flagship_in(c.state, me, system) && can_produce_by_ability(c.state, c.kernel, c.facts, me, system, FLAGSHIP_LIMIT))
}

fn flagship(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(activator)) = (eff.event.payload.system(Key::System), eff.event.payload.seat(Key::Player)) else { return Ok(()) };
    if activator != me || !flagship_in(eff.read().0, me, system) {
        return Ok(());
    }
    produce_in(eff, me, system, FLAGSHIP_LIMIT)?;
    Ok(())
}

/// `commander_ready`: the commander is unlocked, another player activated, and the system holds a unit of the owner with PRODUCTION that
/// could build something.
fn commander_system(s: &CoreState, host: &ti4_engine_core::kernel::Kernel, facts: &ti4_engine_core::kernel::ability::AdapterFacts, owner: Seat, activator: Seat, system: System) -> bool {
    if !leaders::has_commander_ability_in(s, owner, Leader::DirzugaRophal) || activator == owner {
        return false;
    }
    let Some(k) = facts.production_kernel_for(host) else { return false };
    let cx = ti4_engine_core::rules::production::Cx::new(s, k, reach::scope_of(k), reach::blanked(s));
    !ti4_engine_core::rules::production::producers(&cx, owner, system).is_empty() && can_produce_by_ability(s, host, facts, owner, system, 1)
}

fn commander_when(c: &CondCx<'_>) -> bool {
    activated(c).is_some_and(|(activator, system)| commander_system(c.state, c.kernel, c.facts, c.def.owner, activator, system))
}

fn commander(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(activator)) = (eff.event.payload.system(Key::System), eff.event.payload.seat(Key::Player)) else { return Ok(()) };
    if !commander_system(eff.read().0, eff.read().1, eff.adapter_facts(), me, activator, system) {
        return Ok(());
    }
    produce_in(eff, me, system, 1)?;
    Ok(())
}

fn flagship_seat(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| matches!(p.faction, Faction::TheArborec | Faction::TheNekroVirus))
}

/// The abilities of the activation window (`activation::register`): the commander, then the flagship (the string's order, and the flagship of an
/// Arborec or a Nekro seat only).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(COMMANDER, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, commander).optional().when(commander_when));
    t.add(Spec::new(FLAGSHIP, AbilityKind::Unit, EventKind::SystemActivated, Relation::After, flagship).optional().applies(Applies::Held(flagship_seat)).when(flagship_when));
}
