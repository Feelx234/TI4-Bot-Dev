//! mentak: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Salvage Operations, the
//! mandatory half ("After you win or lose a space combat, gain 1 trade good"; a draw is neither). The gain is staged
//! (`TRADE_GOODS_GAINED`, source `mentak`), as the string's `gain_trade_goods_staged`: the window's adapter announces it.
//!
//! And Ipswitch, Loose Cannon (the hero): "At the start of a space combat that you are participating in: you may purge this
//! card. If you do, for each other player's ship that is destroyed during this combat, place 1 ship of that type from your
//! reinforcements in the active system." The purge records the system of the combat (`MentakHero`), each destruction places a
//! ship of the form the owner places, and the end of the combat clears the record.
//!
//! Not ported: the produce half, Pillage, Mentak's agent and commander, Table's Grace.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Leader, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::tech;
use ti4_engine_core::state::{EventKind, Key};

/// Salvage Operations (`technology:*:so:SPACE_COMBAT_ENDED:after`).
pub const SALVAGE_TRADE_GOOD: Slot = Slot(8001);
/// Ipswitch, the purge (`leader:*:mentakhero:SPACE_COMBAT_STARTED:after`).
pub const HERO_START: Slot = Slot(8002);
/// Ipswitch, the ships (`leader:*:mentakhero:SHIP_DESTROYED:after`).
pub const HERO_SHIPS: Slot = Slot(8003);
/// Ipswitch, the end of the combat (`leader:*:mentakhero:SPACE_COMBAT_ENDED:after`).
pub const HERO_END: Slot = Slot(8004);

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let p = &c.event.payload;
    tech::owns(c.state, me, Technology::SalvageOperations)
        && (p.seat(Key::Attacker) == Some(me) || p.seat(Key::Defender) == Some(me))
        && p.seat(Key::Winner).is_some()
}

fn gain(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().mentak;
    fx::gain_trade_goods_staged(eff.state(), me, 1, source);
    Ok(())
}

const HERO: Leader = Leader::IpswitchLooseCannon;

fn took_part(c: &CondCx<'_>, who: Seat) -> bool {
    let p = &c.event.payload;
    p.seat(Key::Attacker) == Some(who) || p.seat(Key::Defender) == Some(who)
}

/// The owner of the record of the combat in `system`, if the hero was purged for it.
fn recorded(s: &ti4_engine_core::state::CoreState, system: ti4_engine_core::generated::System) -> Option<Seat> {
    match s.marks().tail.get(&MarkKey::MentakHero(system)) {
        Some(MarkVal::Atom(Atom::Seat(who))) => Some(*who),
        _ => None,
    }
}

fn start_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    took_part(c, me)
        && c.event.payload.system(Key::System).is_some()
        && c.state.players().by_seat(me).is_some_and(|p| p.leaders.status.get(&HERO) == Some(&ti4_engine_core::state::LeaderStatus::Unlocked))
}

fn purge(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let Some(p) = eff.state().players_mut().by_seat_mut(me) else { return Ok(()) };
    if p.leaders.status.contains_key(&HERO) {
        p.leaders.status.insert(HERO, ti4_engine_core::state::LeaderStatus::Purged);
        eff.state().marks_mut().tail.edit().insert(MarkKey::MentakHero(system), MarkVal::Atom(Atom::Seat(me)));
    }
    Ok(())
}

fn ships_when(c: &CondCx<'_>) -> bool {
    let p = &c.event.payload;
    p.seat(Key::Player).is_some_and(|loser| loser != c.def.owner) && p.system(Key::System).is_some_and(|system| recorded(c.state, system) == Some(c.def.owner))
}

/// One ship of the destroyed type's base type, of the form the owner places, from the reinforcements.
fn place_ship(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(unit)) = (eff.event.payload.system(Key::System), eff.event.payload.unit(Key::Unit)) else { return Ok(()) };
    let scope = ti4_engine_core::tables::Scope(u32::from(eff.kernel().sources.0));
    let Some(base) = scope.unit(unit).map(|d| d.base_type) else { return Ok(()) };
    // The form a seat places for a cruiser (Mentak's Corsair override) and for a flagship (the Keleres variants) is the
    // string's answer (`placed_unit_id`), not the unit tables': the window bails.
    if matches!(base, ti4_engine_core::generated::UnitBaseType::Cruiser | ti4_engine_core::generated::UnitBaseType::Flagship) {
        eff.unknown(ti4_engine_core::kernel::ability::Unknown::MissingFact);
        return Ok(());
    }
    let Some(form) = crate::cd_registry::yin::timing::placed_form(eff.state(), scope, me, base) else { return Ok(()) };
    if ti4_engine_core::query::fleet::remaining(eff.state(), scope, me, form, 0) > 0 {
        fx::place_unit(eff.state(), me, system, None, form);
    }
    Ok(())
}

fn end_when(c: &CondCx<'_>) -> bool {
    c.event.payload.system(Key::System).is_some_and(|system| recorded(c.state, system) == Some(c.def.owner))
}

fn close(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(system) = eff.event.payload.system(Key::System)
        && recorded(eff.state(), system) == Some(me)
    {
        eff.state().marks_mut().tail.edit().remove(&MarkKey::MentakHero(system));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SALVAGE_TRADE_GOOD, AbilityKind::Technology, EventKind::SpaceCombatEnded, Relation::After, gain).when(condition));
    t.add(Spec::new(HERO_START, AbilityKind::Leader, EventKind::SpaceCombatStarted, Relation::After, purge).when(start_when).optional());
    t.add(Spec::new(HERO_SHIPS, AbilityKind::Leader, EventKind::ShipDestroyed, Relation::After, place_ship).when(ships_when));
    t.add(Spec::new(HERO_END, AbilityKind::Leader, EventKind::SpaceCombatEnded, Relation::After, close).when(end_when));
}
