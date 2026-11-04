//! muaat: Adjudicator Ba'al, the hero (lane M6d). "After you move a war sun into a non-home system other than Mecatol Rex: You may destroy all other
//! players' units in that system and replace that system tile with the Muaat supernova tile. If you do, purge this card and each planet card that
//! corresponds to the replaced system tile." Two abilities, as in the string: the record of the war sun's arrival (`muaathero_moved:SHIP_MOVED`)
//! and Nova Seed itself (`muaathero:MOVEMENT_FINISHED`), resolved once movement is finished.
//!
//! The units of other players go through the routes of the core (ships staged as destroyed with the cause `breakthrough:nova_seed`, ground forces
//! on planets staged with the cause `nova_seed`); the tile is replaced on the core's own map (`kernel::fx_map`), the contents of the old system
//! move to the new one, and the planets of the old tile are purged. A position the core cannot carry is a counted bail, never a silent skip:
//! another player's ground forces in the space area (cargo) are announced by the string with the system id in the place of a planet, which the
//! typed event has no field for.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, Planet, System, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Runner, Slot, Spec};
use ti4_engine_core::kernel::fx_map::{self, MapEdit};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, DestroyReason, MarkKey, MarkVal};
use ti4_engine_core::query::{board, fleet, reach};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus, PlanetFlags, SysFlags};

/// Nova Seed (`leader:*:muaathero:MOVEMENT_FINISHED:after`).
pub const NOVA_SEED: Slot = Slot(50_050);
/// The record of a war sun's arrival (`leader:*:muaathero_moved:SHIP_MOVED:after`).
pub const WAR_SUN_MOVED: Slot = Slot(50_051);

const HERO: Leader = Leader::AdjudicatorBaal;

fn is_muaat(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheEmbersOfMuaat)
}

fn hero_unlocked(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&HERO) == Some(&LeaderStatus::Unlocked))
}

/// `base_of(unit) == "warsun"`.
fn has_war_sun(s: &CoreState, k: &Kernel, who: Seat, system: System) -> bool {
    let scope = reach::scope_of(k);
    board::space_units(s, system).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Warsun))
}

fn arrival_condition(c: &CondCx<'_>) -> bool {
    let (s, who) = (c.state, c.def.owner);
    let p = &c.event.payload;
    p.seat(Key::Player) == Some(who) && is_muaat(s, who) && hero_unlocked(s, who) && p.system(Key::System).is_some() && p.unit(Key::Unit).is_some_and(|u| u.data().base_type == UnitBaseType::Warsun)
}

/// "Records, per activation, the system a war sun of the hero's owner moved into."
fn war_sun_moved(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), true) = (eff.event.payload.system(Key::System), eff.event.payload.seat(Key::Player) == Some(me)) else { return Ok(()) };
    let (s, _) = eff.read();
    if !(is_muaat(s, me) && hero_unlocked(s, me)) {
        return Ok(());
    }
    let seq = eff.state().meta().activation_seq;
    eff.state().marks_mut().tail.edit().insert(MarkKey::MuaatWarsunMoved(me), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(system))]));
    Ok(())
}

/// The system Nova Seed would replace for this `MOVEMENT_FINISHED`, if everything about it is legal now (`nova_system`).
fn nova_system(s: &CoreState, k: &Kernel, who: Seat, p: &ti4_engine_core::state::Payload) -> Option<System> {
    if p.seat(Key::Player) != Some(who) || !is_muaat(s, who) || !hero_unlocked(s, who) {
        return None;
    }
    let system = p.system(Key::System)?;
    let seq = s.meta().activation_seq;
    match s.marks().tail.get(&MarkKey::MuaatWarsunMoved(who)) {
        Some(MarkVal::Rec(f)) if matches!(f.as_slice(), [MarkVal::Atom(Atom::Num(n)), MarkVal::Atom(Atom::System(at))] if *n == seq && *at == system) => {}
        _ => return None,
    }
    // Not Mecatol Rex, not a home system (the Fracture is a Thunder's Edge tile: out of the agreed content).
    if system == System::MecatolRex18 || system.data().home_system || !has_war_sun(s, k, who, system) {
        return None;
    }
    // The map can take the tile there (a dry run on a copy).
    let galaxy = s.galaxy()?;
    galaxy.coord_of(ti4_engine_core::ids::system_at(system.index()))?;
    let mut trial = (**galaxy).clone();
    trial.replace_system(ti4_engine_core::ids::system_at(system.index()), ti4_engine_core::ids::system_at(System::NovaSeed.index())).ok()?;
    Some(system)
}

fn nova_when(c: &CondCx<'_>) -> bool {
    nova_system(c.state, c.kernel, c.def.owner, &c.event.payload).is_some()
}

/// `destroy_others`: every other player's ships (staged as destroyed under the cause `breakthrough:nova_seed`), then their ground forces on
/// planets (ground forces announced, structures simply removed), then whatever else is left of theirs in the space area.
fn destroy_others(eff: &mut Eff<'_, '_>, me: Seat, system: System) {
    use ti4_engine_core::rules::combat::{assign::remove_ship, ships_of};
    let cause = eff.codes().activation.nova_seed_cause;
    let others: Vec<Seat> = eff.read().0.players().iter().map(|p| p.seat).filter(|seat| *seat != me).collect();
    let mut doomed = Vec::new();
    {
        let (s, k) = eff.read();
        let blanked = reach::blanked(s);
        for other in &others {
            doomed.extend(ships_of(Rd::new(s, k).with_blanked(blanked), *other, system).into_iter().map(|piece| (*other, piece)));
        }
    }
    eff.inline().with_sx(|sx| {
        for (owner, piece) in doomed {
            remove_ship(sx, system, owner, piece);
            sx.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system, owner, unit: piece.kind(), reason: cause, during_space_combat: false });
        }
    });
    let s = eff.state();
    let mut planets: Vec<Planet> = board::planets_of(s, system).filter(|p| s.board().planet(*p).flags.has(PlanetFlags::UNITS_ENTRY)).collect();
    planets.sort_unstable();
    for planet in planets {
        let victims: Vec<_> = board::planet_units(s, planet).iter().copied().filter(|p| p.owner() != me).collect();
        for piece in victims {
            let units = &mut s.board_mut().planet_mut(planet).units;
            if let Some(at) = units.as_slice().iter().position(|p| *p == piece) {
                units.remove(at);
            }
            if fleet::is_ground_force(piece.kind()) {
                ti4_engine_core::kernel::fx_ground::stage_destroyed(s, system, planet, piece, DestroyReason::NovaSeed);
            }
        }
    }
    // Ground forces of other players left in the space area (cargo) are destroyed too and announced like the ones on planets, in list order;
    // there is no planet, so the system stands in its place (the string's row names the system id there).
    let cargo: Vec<_> = board::space_units(s, system).iter().copied().filter(|p| p.owner() != me && fleet::is_ground_force(p.kind())).collect();
    for piece in cargo {
        ti4_engine_core::kernel::fx_ground::stage_destroyed_in_space(s, system, piece, DestroyReason::NovaSeed);
    }
    s.board_mut().sys_mut(system).units.retain(|p| p.owner() == me);
}

/// `purge_planets`: the planet cards of the replaced tile and everything keyed to them.
fn purge_planets(s: &mut CoreState, system: System) {
    let planets: Vec<Planet> = board::planets_of(s, system).collect();
    for planet in planets {
        ti4_engine_core::rules::explore::purge_planet(s, planet);
        let cell = s.board_mut().planet_mut(planet);
        cell.flags.set(PlanetFlags::EXHAUSTED, false);
        cell.placed_in = None;
        if s.transient().production.value_swapped_planet == Some(planet) {
            s.transient_mut().production.value_swapped_planet = None;
        }
        for i in 0..s.players().len() {
            s.players_mut().edit(i).cards.legendary_exhausted.remove(&planet);
        }
        // Laws that name a planet (Demilitarized Zone, Holy Planet of Ixth, ...) end with the card.
        let named: Vec<_> = s.agenda().laws.iter().filter(|(_, elected)| **elected == ti4_engine_core::keys::Outcome::Planet(planet)).map(|(law, _)| *law).collect();
        for law in named {
            ti4_engine_core::kernel::fx::repeal_law(s, law);
        }
    }
    // Non-faction tokens go with the tile (frontier and command tokens stay).
    s.explore_mut().ingress.remove(system);
    s.explore_mut().breach.remove(system);
    s.board_mut().sys_mut(system).flags.set(SysFlags::PURGED, true);
}

/// `apply_map_edit` for a `Replace`: the contents of the old system (units, command tokens) move to the new one, the frontier token follows, the
/// wormhole tokens and the ion storm of the old tile are gone; the edit is recorded and applied to the core's map.
fn replace_tile(s: &mut CoreState, old: System, new: System) -> bool {
    let planets: Vec<Planet> = board::planets_of(s, old).collect();
    if !fx_map::apply_to_map(s, MapEdit::Replace(old, new)) {
        return false;
    }
    // The string removes the old tile's whole board entry, the `purged_planets` of the planets purged just before with it; only
    // `purged_systems` keeps the tile. The planet cells would otherwise keep a purged flag in a system that is not on the board
    // any more (the position then could not be shown: "planet entry in an untouched system").
    for planet in planets {
        s.board_mut().planet_mut(planet).flags.set(PlanetFlags::PURGED, false);
    }
    let purged = s.board().sys(old).flags.has(SysFlags::PURGED);
    let gone = std::mem::take(s.board_mut().sys_mut(old));
    let moved = s.board_mut().sys_mut(new);
    moved.units = gone.units;
    moved.cmd = gone.cmd;
    moved.flags.set(SysFlags::TOUCHED, true);
    // (The old entry leaves the board; `purged_systems` is the set it was put in, so the flag stays.)
    s.board_mut().sys_mut(old).flags.set(SysFlags::PURGED, purged);
    if s.explore_mut().frontier.remove(old) {
        s.explore_mut().frontier.insert(new);
    }
    let tokens: Vec<_> = s.explore().wormhole_tokens.iter().filter(|(_, at)| **at == old).map(|(kind, _)| *kind).collect();
    for kind in tokens {
        s.explore_mut().wormhole_tokens.remove(&kind);
    }
    if s.meta().ion_storm.is_some_and(|(at, _)| at == old) {
        s.meta_mut().ion_storm = None;
    }
    fx_map::record(s, MapEdit::Replace(old, new));
    true
}

fn nova_seed(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = ({
        let (s, k) = eff.read();
        nova_system(s, k, me, &eff.event.payload)
    }) else {
        return Ok(());
    };
    destroy_others(eff, me, system);
    let s = eff.state();
    purge_planets(s, system);
    if !replace_tile(s, system, System::NovaSeed) {
        return Ok(());
    }
    if s.meta().active_system == Some(system) {
        s.meta_mut().active_system = Some(System::NovaSeed);
    }
    s.marks_mut().tail.edit().remove(&MarkKey::MuaatWarsunMoved(me));
    ti4_engine_core::rules::leaders::purge(s, me, HERO);
    Ok(())
}

/// The record of the arrival, then Nova Seed (the string's order in `muaat::timing_abilities`).
pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(WAR_SUN_MOVED, AbilityKind::Leader, EventKind::ShipMoved, Relation::After, war_sun_moved).when(arrival_condition));
    t.add(Spec::new(NOVA_SEED, AbilityKind::Leader, EventKind::MovementFinished, Relation::After, nova_seed).optional().when(nova_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::galaxy::CoreGalaxy;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::hex::Hex;
    use ti4_engine_core::state::{Piece, PlanetCell};

    /// A table with a map of the tile to replace next to a neighbour, the replaced tile holding another player's planets (controlled, with units).
    fn replaced_tile() -> (CoreState, System, Vec<Planet>) {
        static NO_LANES: [Option<&[(u8, u8)]>; System::COUNT] = [None; System::COUNT];
        let (mut s, _) = table(&[Faction::TheEmbersOfMuaat, Faction::TheFederationOfSol]);
        let plain: Vec<System> = System::ALL.iter().copied().filter(|x| x.data().planets.len() >= 2 && !x.data().hyperlane && !x.data().home_system).take(2).collect();
        let old = plain[0];
        let mut galaxy = CoreGalaxy::new(&NO_LANES);
        galaxy.place(old, Hex::new(0, 0)).unwrap();
        galaxy.place(plain[1], Hex::new(1, 0)).unwrap();
        galaxy.refresh_lane_links();
        s.map_mut().set_galaxy(galaxy);
        let planets: Vec<Planet> = board::planets_of(&s, old).collect();
        for planet in &planets {
            s.board_mut().planet_mut(*planet).control = Some(Seat(1));
            s.board_mut().planet_mut(*planet).units.push(Piece::new(Unit::InfantryI, Seat(1)));
            s.board_mut().planet_mut(*planet).flags.set(PlanetFlags::UNITS_ENTRY, true);
            s.board_mut().planet_mut(*planet).flags.set(PlanetFlags::EXHAUSTED, true);
        }
        s.board_mut().sys_mut(old).flags.set(SysFlags::TOUCHED, true);
        s.board_mut().sys_mut(old).units.push(Piece::new(Unit::WarSun, Seat(0)));
        (s, old, planets)
    }

    /// Found by the rule bot's games (4p seed 5, 5p seed 5, 6p seeds 3 to 5): after Nova Seed the position could not be shown ("planet entry in an
    /// untouched system"): the replaced tile's planets kept their `PURGED` flag while the tile's board entry was gone. The string removes the whole
    /// entry (`purged_planets` with it); only `purged_systems` keeps the tile.
    #[test]
    fn a_replaced_tile_leaves_no_planet_entry_behind_and_the_position_still_shows() {
        let (mut s, old, planets) = replaced_tile();
        purge_planets(&mut s, old);
        assert!(planets.iter().all(|p| s.board().planet(*p).flags.has(PlanetFlags::PURGED)), "purged by the card, until the tile goes");
        assert!(replace_tile(&mut s, old, System::NovaSeed));
        for planet in &planets {
            assert_eq!(s.board().planet(*planet), &PlanetCell::default(), "{planet:?}");
        }
        let gone = s.board().sys(old);
        assert!(gone.flags.has(SysFlags::PURGED) && !gone.flags.has(SysFlags::TOUCHED) && gone.units.is_empty(), "only purged_systems keeps the tile");
        let new = s.board().sys(System::NovaSeed);
        assert!(new.flags.has(SysFlags::TOUCHED) && new.units.iter().any(|p| p.kind() == Unit::WarSun), "the war sun moved to the supernova");
    }
}
