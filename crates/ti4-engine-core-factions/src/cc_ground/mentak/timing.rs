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
use ti4_engine_core::generated::{Leader, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx_pay;
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::tech;
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload};

/// Salvage Operations (`technology:*:so:SPACE_COMBAT_ENDED:after`).
pub const SALVAGE_TRADE_GOOD: Slot = Slot(8001);
/// Salvage Operations, the produce half (`technology:*:so:SPACE_COMBAT_ENDED:produce`; the registered relation is `after`).
pub const SALVAGE_SHIP: Slot = Slot(8005);
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

// -- Salvage Operations, the produce half --------------------------------------------------------------

/// The ships Salvage Operations could produce now, in the string's order (`buildable_for`): the ship types destroyed in the
/// combat, that `owner` may still place and can pay for. `Err`: a fact is missing (the buildable list, a payment kernel) or the
/// seat holds ocean cards.
fn salvage_options(s: &CoreState, f: &AdapterFacts, owner: Seat, p: &Payload) -> Result<Vec<(Unit, i64)>, ()> {
    use ti4_engine_core::generated::UnitBaseType;
    use ti4_engine_core::query::{economy::Spend, objectives as q};
    if !tech::owns(s, owner, Technology::SalvageOperations) || p.seat(Key::Winner) != Some(owner) || (p.seat(Key::Attacker) != Some(owner) && p.seat(Key::Defender) != Some(owner)) {
        return Ok(Vec::new());
    }
    let destroyed: Vec<UnitBaseType> = match p.list(Key::Destroyed) {
        Some(ti4_engine_core::state::List::Units(units)) => units.iter().filter(|(_, u)| u.data().ship).map(|(_, u)| u.data().base_type).collect(),
        _ => Vec::new(),
    };
    let buildable = f.buildable(owner).ok_or(())?;
    let k = f.pay_kernel.ok_or(())?();
    let scope = ti4_engine_core::tables::Scope(u32::from(k.sources.0));
    let env = fx_pay::PayEnv { scope, blanked: ti4_engine_core::query::reach::blanked(s) };
    let facts = fx_pay::pay_facts(s, k, env, owner);
    if facts.terms.oceans {
        return Err(());
    }
    let mut out = Vec::new();
    for unit in buildable.iter().copied() {
        let data = unit.data();
        if !(data.ship && destroyed.contains(&data.base_type)) || ti4_engine_core::query::fleet::remaining_all(s, scope, owner, unit) <= 0 {
            continue;
        }
        let cost = (i64::from(data.cost_halves.unwrap_or(0)) + 1) / 2;
        if q::first_plan(s, &facts, Spend::Resources, cost).is_some() {
            out.push((unit, cost));
        }
    }
    Ok(out)
}

fn salvage_when(c: &CondCx<'_>) -> bool {
    match salvage_options(c.state, c.facts, c.def.owner, &c.event.payload) {
        Ok(found) => !found.is_empty(),
        Err(()) => c.unknown(ti4_engine_core::kernel::ability::Unknown::MissingFact),
    }
}

fn salvage(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Pick, Subtype};
    let me = eff.owner();
    let (Ok(found), Some(system)) = (salvage_options(eff.read().0, eff.adapter_facts(), me, &eff.event.payload), eff.event.payload.system(Key::System)) else { return Ok(()) };
    if found.is_empty() {
        return Ok(());
    }
    let mut opts: Opts = found.iter().map(|(unit, cost)| Opt::Reaction(ROpt::Produce { unit: *unit, cost: i16::try_from(*cost).unwrap_or(i16::MAX) })).collect();
    opts.push(Opt::Decline);
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::Salvage),
        ctx: Ctx {
            origin: ti4_engine_core::choice::Origin::FactionAbility(ti4_engine_core::choice::generic::Content::Other(eff.codes().salvage)),
            subtype: Subtype::Reaction(RSub::Salvage),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    let Pick::Opt(Opt::Reaction(ROpt::Produce { unit, cost })) = eff.ask(&choice) else { return Ok(()) };
    let base = unit.data().base_type;
    // The form a seat places is `placed_unit_id` with the override of the hook table (Mentak's Corsair for a Cruiser II).
    let before = eff.state().clone();
    let paid = ti4_engine_core::rules::production::pay::pay_in(eff, me, i64::from(cost), ti4_engine_core::query::economy::Spend::Resources)?;
    let scope = ti4_engine_core::tables::Scope(u32::from(eff.kernel().sources.0));
    let form = {
        let (state, kernel) = eff.read();
        crate::cd_registry::yin::timing::placed_form_overridden(state, kernel, scope, me, base)
    };
    let placed = paid
        && form
            .filter(|form| ti4_engine_core::query::fleet::remaining_all(eff.state(), scope, me, *form) > 0)
            .map(|form| fx::place_unit(eff.state(), me, system, None, form))
            .is_some();
    if !placed {
        *eff.state() = before;
    }
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
    // The form a seat places is `placed_unit_id` with the override of the hook table (Mentak's Corsair for a Cruiser II).
    let form = {
        let (state, kernel) = eff.read();
        crate::cd_registry::yin::timing::placed_form_overridden(state, kernel, scope, me, base)
    };
    let Some(form) = form else { return Ok(()) };
    if ti4_engine_core::query::fleet::remaining_all(eff.state(), scope, me, form) > 0 {
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
    t.add(Spec::new(SALVAGE_SHIP, AbilityKind::Technology, EventKind::SpaceCombatEnded, Relation::After, salvage).when(salvage_when).optional());
    t.add(Spec::new(HERO_START, AbilityKind::Leader, EventKind::SpaceCombatStarted, Relation::After, purge).when(start_when).optional());
    t.add(Spec::new(HERO_SHIPS, AbilityKind::Leader, EventKind::ShipDestroyed, Relation::After, place_ship).when(ships_when));
    t.add(Spec::new(HERO_END, AbilityKind::Leader, EventKind::SpaceCombatEnded, Relation::After, close).when(end_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::{Faction, System};
    use ti4_engine_core::state::{List, Piece, Val};

    fn plain_kernel() -> &'static ti4_engine_core::kernel::Kernel {
        Box::leak(Box::new(ti4_engine_core::kernel::Kernel::plain()))
    }

    /// The Mentak (seat 0) won against a Hacan; the Hacan lost a dreadnought and a destroyer; the Mentak may build both.
    fn setup() -> (CoreState, AdapterFacts, Payload) {
        let (mut s, _) = table(&[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SalvageOperations);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 6;
        let mut f = AdapterFacts::default();
        f.pay_kernel = Some(plain_kernel);
        f.set_buildable(Seat(0), vec![Unit::DestroyerI, Unit::DreadnoughtI, Unit::InfantryI]);
        let p = Payload::new()
            .with(Key::Attacker, Val::Seat(Seat(0)))
            .with(Key::Defender, Val::Seat(Seat(1)))
            .with(Key::Winner, Val::Seat(Seat(0)))
            .with_list(Key::Destroyed, List::Units([(Seat(1), Unit::DreadnoughtI), (Seat(1), Unit::DestroyerI)].into_iter().collect()));
        (s, f, p)
    }

    /// Package W3: the form a Mentak seat places for a cruiser (Salvage Operations, Ipswitch) is the hook table's: the plain cruiser,
    /// Cruiser II with the technology, the Corsair with The Table's Grace on top of it.
    #[test]
    fn the_form_a_mentak_seat_places_for_a_cruiser_follows_the_hook_table() {
        use ti4_engine_core::generated::{Breakthrough, Technology as T, UnitBaseType};
        use ti4_engine_core::kernel::Kernel;
        let (mut s, k) = table(&[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan]);
        let kernel = {
            let mut hooks = ti4_engine_core::hooks::HookTable::new();
            crate::cc_ground::mentak::query::register(&mut hooks);
            Kernel { hooks, ..k }
        };
        let scope = ti4_engine_core::tables::Scope(u32::MAX);
        let form = |s: &CoreState| crate::cd_registry::yin::timing::placed_form_overridden(s, &kernel, scope, Seat(0), UnitBaseType::Cruiser);
        assert_eq!(form(&s), Some(Unit::CruiserI));
        s.players_mut().edit(0).tech.owned.insert(T::CruiserII);
        assert_eq!(form(&s), Some(Unit::CruiserII));
        s.players_mut().edit(0).cards.breakthrough = Some(Breakthrough::TheTablesGrace);
        assert_eq!(form(&s), Some(Unit::Corsair));
    }

    #[test]
    fn salvage_offers_the_destroyed_ship_types_the_owner_may_build_and_pay() {
        let (mut s, f, p) = setup();
        assert_eq!(salvage_options(&s, &f, Seat(0), &p), Ok(vec![(Unit::DestroyerI, 1), (Unit::DreadnoughtI, 4)]), "ships only, in the string's buildable order");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 2;
        assert_eq!(salvage_options(&s, &f, Seat(0), &p), Ok(vec![(Unit::DestroyerI, 1)]), "the dreadnought is out of reach of 2 resources");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 6;
        let loser = p.clone().with(Key::Winner, Val::Seat(Seat(1)));
        assert_eq!(salvage_options(&s, &f, Seat(0), &loser), Ok(Vec::new()), "only the winner produces");
        let outsider = p.clone().with(Key::Attacker, Val::Seat(Seat(1)));
        assert_eq!(salvage_options(&s, &f, Seat(0), &outsider), Ok(Vec::new()), "the owner fought in it");
        for _ in 0..5 {
            s.board_mut().sys_mut(System::ALL[0]).units.push(Piece::new(Unit::DreadnoughtI, Seat(0)));
        }
        assert_eq!(salvage_options(&s, &f, Seat(0), &p), Ok(vec![(Unit::DestroyerI, 1)]), "no dreadnought plastic left");
        assert_eq!(salvage_options(&s, &AdapterFacts::default(), Seat(0), &p), Err(()), "no buildable list or payment kernel lent");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(Technology::SalvageOperations);
        assert_eq!(salvage_options(&s, &f, Seat(0), &p), Ok(Vec::new()), "without the technology");
    }
}
