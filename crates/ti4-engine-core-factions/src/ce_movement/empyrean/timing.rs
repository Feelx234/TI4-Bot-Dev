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
use ti4_engine_core::generated::{Faction, Leader, Planet, System, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
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

/// Aether Passage (`ability:*:aetherpassage:SYSTEM_ACTIVATED:after`).
pub const PASSAGE: Slot = Slot(13004);
/// Aetherstream (`technology:*:as:SYSTEM_ACTIVATED:after`).
pub const STREAM: Slot = Slot(13005);

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
fn target(s: &CoreState, p: &Payload) -> Option<Target> {
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
/// The sustained unit Dynamo may repair: its system, planet, owner and type.
type Target = (System, Option<Planet>, Seat, Unit);

fn window(s: &CoreState, f: &AdapterFacts, owner: Seat, p: &Payload) -> Result<Option<Target>, ()> {
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

// -- Aether Passage and Aetherstream (the activation window, lane M6c) ---------------------------------

/// `has_ship_outside`: `who` has a ship in some system other than `except`.
fn ship_outside(s: &CoreState, who: Seat, except: System) -> bool {
    s.board().systems().any(|(system, cell)| system != except && cell.units.as_slice().iter().any(|u| u.owner() == who && u.kind().data().ship))
}

/// `passage_ready`: the activator is another player, both seats have a ship outside the activated system, and `owner` is the Empyrean.
fn passage_ready(s: &CoreState, owner: Seat, p: &Payload) -> Option<Seat> {
    let (player, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    (s.players().by_seat(owner).is_some_and(|y| y.faction == Faction::TheEmpyrean) && player != owner && ship_outside(s, owner, system) && ship_outside(s, player, system)).then_some(player)
}

fn passage_when(c: &CondCx<'_>) -> bool {
    passage_ready(c.state, c.def.owner, &c.event.payload).is_some()
}

/// "You may allow that player to move their ships through systems that contain your ships": the permission, stamped with this activation.
fn passage(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let p = eff.event.payload.clone();
    let Some(player) = passage_ready(eff.state(), owner, &p) else { return Ok(()) };
    let seq = eff.state().meta().activation_seq;
    eff.state().marks_mut().tail.edit().insert(MarkKey::EmpyreanPassage(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Seat(player)), MarkVal::Atom(Atom::Seat(owner))]));
    Ok(())
}

/// `transactions::are_neighbours` for two seats of the position.
fn are_neighbours(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, a: Seat, b: Seat) -> bool {
    use ti4_engine_core::query::trade::{self, Cx, Limits};
    if !s.loaded.has(ti4_engine_core::state::Slice::Map) {
        return false;
    }
    let Some(galaxy) = s.galaxy() else { return false };
    let adj = galaxy.adjacency();
    let mecatol = if galaxy.coord_of(System::MecatolRex112).is_some() { System::MecatolRex112 } else { System::MecatolRex18 };
    let env = ti4_engine_core::rules::trade::KernelEnv { core: s, kernel: k };
    trade::are_neighbours(&Cx { core: s, adj: &adj, mecatol, limits: Limits { per_action: 2, per_round: 6 }, env: &env }, a, b)
}

/// `stream_ready`: the owner has the technology, the activator is the owner or a neighbour with a ship outside the system, and the system is
/// next to an anomaly as the activator sees the map (false without a map, as the string's `context.galaxy` is `None`).
fn stream_ready(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, owner: Seat, p: &Payload) -> Result<Option<Seat>, ()> {
    let (Some(player), Some(system)) = (p.seat(Key::Player), p.system(Key::System)) else { return Ok(None) };
    if !ti4_engine_core::query::reach::technology_text(s, owner, Technology::Aetherstream) || !(player == owner || are_neighbours(s, k, owner, player)) || !ship_outside(s, player, system) {
        return Ok(None);
    }
    let anomaly = ti4_engine_core::query::reach::adjacent_to_anomaly(s, k, player, system).unwrap_or(false);
    Ok(anomaly.then_some(player))
}

fn stream_when(c: &CondCx<'_>) -> bool {
    match stream_ready(c.state, c.kernel, c.def.owner, &c.event.payload) {
        Ok(found) => found.is_some(),
        Err(()) => c.unknown(Unknown::MissingFact),
    }
}

/// "+1 to the move value of all of that player's ships during this tactical action": the stamp `move_bonus` reads.
fn stream(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let found = {
        let (state, kernel) = eff.read();
        stream_ready(state, kernel, owner, &eff.event.payload)
    };
    let Ok(Some(player)) = found else { return Ok(()) };
    let seq = eff.state().meta().activation_seq;
    eff.state().marks_mut().tail.edit().insert(MarkKey::EmpyreanAetherstream(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Seat(player))]));
    Ok(())
}

/// The two abilities of the activation window, in the string's order (`activation::register`).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(PASSAGE, AbilityKind::Ability, EventKind::SystemActivated, Relation::After, passage).optional().when(passage_when));
    t.add(Spec::new(STREAM, AbilityKind::Technology, EventKind::SystemActivated, Relation::After, stream).optional().when(stream_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::state::{Piece, Val};

    fn event(system: System, owner: Seat, kind: Unit) -> Payload {
        Payload::new().with(Key::System, Val::System(system)).with(Key::Player, Val::Seat(owner)).with(Key::Unit, Val::Unit(kind))
    }

    #[test]
    fn the_target_is_a_damaged_unit_of_the_named_owner_and_type() {
        let (mut s, _) = table(&[Faction::TheEmiratesOfHacan, Faction::TheEmpyrean]);
        let system = System::ALL[0];
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::DreadnoughtI, Seat(0)));
        assert_eq!(target(&s, &event(system, Seat(0), Unit::DreadnoughtI)), None, "an undamaged unit has nothing to repair");
        s.board_mut().sys_mut(system).units.as_mut_slice()[0] = Piece::new(Unit::DreadnoughtI, Seat(0)).with_sustained(true);
        assert_eq!(target(&s, &event(system, Seat(0), Unit::DreadnoughtI)), Some((system, None, Seat(0), Unit::DreadnoughtI)));
        assert_eq!(target(&s, &event(system, Seat(1), Unit::DreadnoughtI)), None, "another player's unit");
        assert_eq!(target(&s, &event(system, Seat(0), Unit::CruiserI)), None, "another type");
    }

    #[test]
    fn the_map_is_an_input_and_a_pair_the_string_did_not_ask_makes_the_answer_unknown() {
        let mut f = AdapterFacts::default();
        let (a, b, c) = (System::ALL[0], System::ALL[1], System::ALL[2]);
        assert_eq!(any_near(&f, [a].into_iter(), a, true), Ok(true), "the same system needs no map");
        assert_eq!(any_near(&f, [b].into_iter(), a, true), Err(()), "never asked");
        f.set_adjacent(b, a, false);
        assert_eq!(any_near(&f, [b].into_iter(), a, true), Ok(false));
        f.set_adjacent(c, a, true);
        assert_eq!(any_near(&f, [b, c].into_iter(), a, true), Ok(true));
        assert_eq!(any_near(&f, [c].into_iter(), b, false), Err(()), "the pair is asked in the string's order (system, candidate)");
    }

    fn plain_kernel() -> &'static ti4_engine_core::kernel::Kernel {
        Box::leak(Box::new(ti4_engine_core::kernel::Kernel::plain()))
    }

    /// A window that follows nested activity is the only one that arms the table, so a condition that wrongly says yes cannot
    /// show in a lockstep test of a window without activity: these are the pure checks of the conditions.
    #[test]
    fn dynamo_wants_two_influence_the_seat_can_pay() {
        let (mut s, _) = table(&[Faction::TheEmpyrean, Faction::TheEmiratesOfHacan]);
        let mut f = AdapterFacts::default();
        f.pay_kernel = Some(plain_kernel);
        assert_eq!(affordable(&s, &f, Seat(0)), Ok(false), "no planet, no trade good");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 1;
        assert_eq!(affordable(&s, &f, Seat(0)), Ok(false), "one trade good is one influence");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 2;
        assert_eq!(affordable(&s, &f, Seat(0)), Ok(true));
        assert_eq!(affordable(&s, &AdapterFacts::default(), Seat(0)), Err(()), "no payment kernel lent: unknown");
    }

    #[test]
    fn the_watcher_is_for_another_players_card_and_a_mech_in_reach() {
        let (mut s, _) = table(&[Faction::TheEmpyrean, Faction::TheEmiratesOfHacan]);
        let (home, there) = (System::ALL[0], System::ALL[1]);
        s.board_mut().sys_mut(there).units.push(Piece::new(Unit::Watcher, Seat(0)));
        s.board_mut().sys_mut(home).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        let played = |by: Seat| Payload::new().with(Key::Player, Val::Seat(by));
        let mut f = AdapterFacts::default();
        f.set_adjacent(there, home, true);
        assert_eq!(watcher_window(&s, &f, Seat(0), &played(Seat(1))), Ok(Some((Seat(1), vec![(there, None)]))));
        assert_eq!(watcher_window(&s, &f, Seat(0), &played(Seat(0))), Ok(None), "not for the Empyrean's own card");
        let mut far = AdapterFacts::default();
        far.set_adjacent(there, home, false);
        assert_eq!(watcher_window(&s, &far, Seat(0), &played(Seat(1))), Ok(None), "out of reach");
        assert_eq!(watcher_window(&s, &AdapterFacts::default(), Seat(0), &played(Seat(1))), Err(()), "the pair was not asked");
    }
}

// -- Xuange, the Empyrean commander (lane M6d) ----------------------------------------------------------------------------------------

/// Xuange (`leader:*:empyreancommander:SHIP_MOVED:after`; registered for every seat, a commander can be held through an Alliance): "After another
/// player moves ships into a system that contains 1 of your command tokens: you may return that token to your reinforcements."
pub const XUANGE: Slot = Slot(50_070);

/// `empyrean_ready`: another player's ships entered a system holding a command token of `owner`, who has the commander's ability.
fn xuange_ready(c: &CondCx<'_>) -> Option<System> {
    let p = &c.event.payload;
    let (mover, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    (mover != c.def.owner && ti4_engine_core::query::leaders::has_commander_ability_in(c.state, c.def.owner, Leader::Xuange) && c.state.board().sys(system).cmd.contains(c.def.owner)).then_some(system)
}

fn xuange_when(c: &CondCx<'_>) -> bool {
    xuange_ready(c).is_some()
}

fn xuange(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let p = &eff.event.payload;
    let (Some(mover), Some(system)) = (p.seat(Key::Player), p.system(Key::System)) else { return Ok(()) };
    let s = eff.state();
    if mover != me && ti4_engine_core::query::leaders::has_commander_ability_in(s, me, Leader::Xuange) && s.board().sys(system).cmd.contains(me) {
        s.board_mut().sys_mut(system).cmd.remove(me);
    }
    Ok(())
}

// -- Acamar, the Empyrean agent (lane M6d) ---------------------------------------------------------------------------------------------

/// Acamar (`leader:*:empyreanagent:MOVEMENT_FINISHED:after`): "after a player moves ships into a system with no planets: you may exhaust this card; that
/// player gains 1 command token", into the pool they choose.
pub const ACAMAR: Slot = Slot(50_071);

fn acamar_mover(c: &CondCx<'_>) -> Option<Seat> {
    let (s, me) = (c.state, c.def.owner);
    if s.players().by_seat(me)?.leaders.status.get(&Leader::Acamar) != Some(&ti4_engine_core::state::LeaderStatus::Readied) {
        return None;
    }
    let p = &c.event.payload;
    if !matches!(p.get(Key::ShipsMoved), Some(ti4_engine_core::state::Val::Int(n)) if n > 0) {
        return None;
    }
    let (mover, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    (s.players().by_seat(mover).is_some() && system.data().planets.is_empty()).then_some(mover)
}

fn acamar_when(c: &CondCx<'_>) -> bool {
    acamar_mover(c).is_some()
}

/// The agent is exhausted, then the mover answers the pool question (the string restores everything when the answer is refused).
fn acamar(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::strategy::{gain_pool_choice, Opt as SOpt};
    use ti4_engine_core::choice::{Opt, Pick};
    let me = eff.owner();
    let p = &eff.event.payload;
    let (Some(mover), Some(system), true) = (p.seat(Key::Player), p.system(Key::System), matches!(p.get(Key::ShipsMoved), Some(ti4_engine_core::state::Val::Int(n)) if n > 0)) else { return Ok(()) };
    let s = eff.state();
    if s.players().by_seat(me).and_then(|pl| pl.leaders.status.get(&Leader::Acamar).copied()) != Some(ti4_engine_core::state::LeaderStatus::Readied) || s.players().by_seat(mover).is_none() || !system.data().planets.is_empty() {
        return Ok(());
    }
    let (state, kernel) = eff.state_and_kernel();
    if !ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, me, Leader::Acamar) {
        return Ok(());
    }
    if let Pick::Opt(Opt::Strategy(SOpt::Pool(pool))) = eff.ask(&gain_pool_choice(mover, 1)) {
        ti4_engine_core::kernel::fx::gain_token(eff.state(), mover, pool, 1);
    }
    Ok(())
}

/// The commander's ability (after the faction modules, as `borrowed_commanders_b` is).
pub fn register_late(t: &mut AbilityTable) {
    t.add(Spec::new(XUANGE, AbilityKind::Leader, EventKind::ShipMoved, Relation::After, xuange).optional().when(xuange_when));
}

/// Acamar (the Empyrean module of the string: before the borrowed commanders).
pub fn register_agent(t: &mut AbilityTable) {
    t.add(Spec::new(ACAMAR, AbilityKind::Leader, EventKind::MovementFinished, Relation::After, acamar).optional().when(acamar_when));
}
