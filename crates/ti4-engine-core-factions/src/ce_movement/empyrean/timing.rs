//! empyrean: the timing abilities of the flagship (Dynamo) and the mech (Watcher), docs/ENGINE_REACTIONS.md (package B12).
//! Registered for every seat, as the string does (the conditions name the units).
//!
//! * Dynamo: "after any player's unit in this system or an adjacent system uses SUSTAIN DAMAGE, you may spend 2 influence to
//!   repair that unit". Optional; the planets paid are the payment's own questions (`pay_in`).
//! * The Watcher: "you may remove this unit from a system that contains or is adjacent to another player's units to cancel
//!   an action card played by that player". WHEN `ACTION_CARD_PLAYED`, optional; a question when several mechs are in reach.
//!
//! The map is an input: the string asks `galaxy.adjacent`, the core is lent no galaxy, so the string's `near` notes each
//! pair it asked (`AdapterFacts::adjacent`) and a pair it did not ask makes the window unknown (`MissingFact`).
//! A Nekro flagship may carry Dynamo through the Z token (`common::flagship_has_text`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::fx_pay::{pay_facts, PayEnv};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{board, economy::Spend, objectives as q};
use ti4_engine_core::rules::production::pay::pay_in;
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload};
use ti4_engine_core::tables::Scope;

use crate::common::flagship_has_text;

/// Dynamo, a ship (`unit:*:empyrean_flagship:SUSTAIN_DAMAGE_USED:after`).
pub const DYNAMO_SHIP: Slot = Slot(13001);
/// Dynamo, a ground force (`unit:*:empyrean_flagship:GROUND_FORCE_SUSTAINED:after`).
pub const DYNAMO_GROUND: Slot = Slot(13002);
/// The Watcher (`unit:*:empyrean_mech:ACTION_CARD_PLAYED:when`).
pub const WATCHER: Slot = Slot(13003);

const COST: i64 = 2;

/// What the map says of `a` and `b` as the string's `near` asked it: `Some(true)` when the same system or adjacent.
fn near(f: &AdapterFacts, a: System, b: System) -> Option<bool> {
    if a == b { Some(true) } else { f.adjacent(a, b) }
}

/// Whether any of `candidates` is near `system`, asking the pairs in the order `first_is_candidate` says (Dynamo asks
/// `near(at, system)`, the Watcher `near(system, other)`); `Err` when the string did not ask a pair the answer needs.
fn any_near(f: &AdapterFacts, candidates: impl Iterator<Item = System>, system: System, first_is_candidate: bool) -> Result<bool, ()> {
    let mut missing = false;
    for at in candidates {
        let (a, b) = if first_is_candidate { (at, system) } else { (system, at) };
        match near(f, a, b) {
            Some(true) => return Ok(true),
            Some(false) => {}
            None => missing = true,
        }
    }
    if missing { Err(()) } else { Ok(false) }
}

/// The sustained unit the event names, if it is still on the board and damaged.
fn target(s: &CoreState, p: &Payload) -> Option<(System, Option<Planet>, Seat, Unit)> {
    let (system, owner, kind) = (p.system(Key::System)?, p.seat(Key::Player)?, p.unit(Key::Unit)?);
    let planet = p.planet(Key::Planet);
    let pieces = match planet {
        Some(planet) => board::planet_units(s, planet),
        None => board::space_units(s, system),
    };
    pieces.iter().any(|u| u.owner() == owner && u.kind() == kind && u.sustained()).then_some((system, planet, owner, kind))
}

/// Systems where `owner` has a ship that carries Dynamo's text.
fn flagship_systems(s: &CoreState, owner: Seat) -> impl Iterator<Item = System> + '_ {
    System::ALL.iter().copied().filter(move |sys| board::space_units(s, *sys).iter().any(|u| u.owner() == owner && flagship_has_text(s, owner, u.kind(), Unit::Dynamo)))
}

/// Whether `owner` can pay 2 influence now (`payment::affordable`): a plan exists. `Err` while the seat holds ocean cards
/// (the string pays with those too) or no payment kernel was lent.
fn affordable(s: &CoreState, f: &AdapterFacts, owner: Seat) -> Result<bool, ()> {
    let k = f.pay_kernel.ok_or(())?();
    let env = PayEnv { scope: Scope(u32::from(k.sources.0)), blanked: ti4_engine_core::query::reach::blanked(s) };
    let facts = pay_facts(s, k, env, owner);
    if facts.terms.oceans {
        return Err(());
    }
    Ok(q::first_plan(s, &facts, Spend::Influence, COST).is_some())
}

/// The repair Dynamo offers `owner` for the event: the unit is damaged, the flagship reaches it and 2 influence can be paid.
fn window(s: &CoreState, f: &AdapterFacts, owner: Seat, p: &Payload) -> Result<Option<(System, Option<Planet>, Seat, Unit)>, ()> {
    let Some(t) = target(s, p) else { return Ok(None) };
    if !any_near(f, flagship_systems(s, owner), t.0, true)? {
        return Ok(None);
    }
    Ok(affordable(s, f, owner)?.then_some(t))
}

fn dynamo_when(c: &CondCx<'_>) -> bool {
    match window(c.state, c.facts, c.def.owner, &c.event.payload) {
        Ok(found) => found.is_some(),
        Err(()) => c.unknown(Unknown::MissingFact),
    }
}

fn dynamo(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let found = window(eff.read().0, eff.adapter_facts(), me, &eff.event.payload);
    let Ok(Some((system, planet, owner, kind))) = found else { return Ok(()) };
    let before = eff.state().clone();
    let paid = pay_in(eff, me, COST, Spend::Influence)?;
    if paid && repair(eff.state(), system, planet, owner, kind) {
        return Ok(());
    }
    *eff.state() = before;
    Ok(())
}

/// Mark the first damaged matching unit repaired. `false` (nothing changed) if there is none.
fn repair(s: &mut CoreState, system: System, planet: Option<Planet>, owner: Seat, kind: Unit) -> bool {
    let cell = match planet {
        Some(planet) => &mut s.board_mut().planet_mut(planet).units,
        None => &mut s.board_mut().sys_mut(system).units,
    };
    let Some(piece) = cell.as_mut_slice().iter_mut().find(|u| u.owner() == owner && u.kind() == kind && u.sustained()) else { return false };
    *piece = piece.with_sustained(false);
    true
}

// -- the Watcher ---------------------------------------------------------------------------------------

type Place = (System, Option<Planet>);

/// The places `owner`'s mechs stand in a system that contains or is next to a unit of `actor`, in the string's order (system
/// order, the space area before the planets).
fn sites(s: &CoreState, f: &AdapterFacts, owner: Seat, actor: Seat) -> Result<Vec<Place>, ()> {
    let has = |u: &ti4_engine_core::state::Piece| u.owner() == actor;
    let occupied: Vec<System> = System::ALL
        .iter()
        .copied()
        .filter(|sys| board::space_units(s, *sys).iter().any(has) || board::planets_of(s, *sys).any(|pl| board::planet_units(s, pl).iter().any(has)))
        .collect();
    let mut out = Vec::new();
    for sys in System::ALL.iter().copied() {
        let is_mech = |u: &ti4_engine_core::state::Piece| u.owner() == owner && u.kind() == Unit::Watcher;
        let space = board::space_units(s, sys).iter().any(is_mech);
        let mut planets: Vec<Planet> = board::planets_of(s, sys).filter(|pl| board::planet_units(s, *pl).iter().any(is_mech)).collect();
        planets.sort_unstable();
        if !space && planets.is_empty() {
            continue;
        }
        if !any_near(f, occupied.iter().copied(), sys, false)? {
            continue;
        }
        if space {
            out.push((sys, None));
        }
        out.extend(planets.into_iter().map(|pl| (sys, Some(pl))));
    }
    Ok(out)
}

fn watcher_window(s: &CoreState, f: &AdapterFacts, owner: Seat, p: &Payload) -> Result<Option<(Seat, Vec<Place>)>, ()> {
    let Some(actor) = p.seat(Key::Player) else { return Ok(None) };
    if actor == owner || s.players().by_seat(actor).is_none() {
        return Ok(None);
    }
    let found = sites(s, f, owner, actor)?;
    Ok((!found.is_empty()).then_some((actor, found)))
}

fn watcher_when(c: &CondCx<'_>) -> bool {
    match watcher_window(c.state, c.facts, c.def.owner, &c.event.payload) {
        Ok(found) => found.is_some(),
        Err(()) => c.unknown(Unknown::MissingFact),
    }
}

fn watcher(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let found = watcher_window(eff.read().0, eff.adapter_facts(), me, &eff.event.payload);
    let Ok(Some((actor, places))) = found else { return Ok(()) };
    let place = if let [only] = places.as_slice() {
        *only
    } else {
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::Watcher { actor }),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Other(eff.codes().empyrean_mech)),
                subtype: Subtype::Reaction(RSub::Watcher),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: places.iter().map(|(system, planet)| Opt::Reaction(ROpt::Site { system: *system, planet: *planet })).collect::<Opts>(),
        };
        match eff.ask(&choice) {
            Pick::Opt(Opt::Reaction(ROpt::Site { system, planet })) => (system, planet),
            _ => return Ok(()),
        }
    };
    let cell = match place.1 {
        Some(planet) => &mut eff.state().board_mut().planet_mut(planet).units,
        None => &mut eff.state().board_mut().sys_mut(place.0).units,
    };
    if let Some(i) = cell.as_slice().iter().position(|u| u.owner() == me && u.kind() == Unit::Watcher) {
        cell.remove(i);
        eff.cancel();
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(DYNAMO_SHIP, AbilityKind::Unit, EventKind::SustainDamageUsed, Relation::After, dynamo).when(dynamo_when).optional());
    t.add(Spec::new(DYNAMO_GROUND, AbilityKind::Unit, EventKind::GroundForceSustained, Relation::After, dynamo).when(dynamo_when).optional());
    t.add(Spec::new(WATCHER, AbilityKind::Unit, EventKind::ActionCardPlayed, Relation::When, watcher).when(watcher_when).optional());
}
