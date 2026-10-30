//! Space combat (LRR 78, with 87 governing Sustain Damage): the typed port of `ti4-engine/src/combat.rs`
//! (package B1).
//!
//! Layout of this file, in the order of the string module:
//!
//! * **no faction seam of its own**: every rule the string combat takes from faction code, a law, a relic
//!   or a card is read from the kernel's uniform seams (`k.hooks.seam`: `hit_shift`, `dice`, `may_sustain`,
//!   `borrowed_stats`, `alastor_participants`, and the data rows), see `kernel/seam`;
//! * queries over the board: [`combatants`], [`ships_of`], [`hits_on`], [`effective_hits_on`];
//! * the fleet's dice: [`fleet_groups`], [`roll_fleet`];
//! * retreats, cancellations, anti-fighter barrage and space cannon follow in the later sections.
//!
//! Nothing here asks a question or emits an event: those go through the host the later sections take.

pub mod assault_cannon;
pub mod assign;
pub mod barrage;
pub mod begin_round;
pub mod cannon_guns;
pub mod destroy_fighters;
pub mod effect_hits;
pub mod end;
pub mod end_announce;
pub mod feats;
pub mod flow;
pub mod machine;
pub mod raid_formation;
pub mod retreat;
pub mod roll;
pub mod sustain_commander;
pub mod space_cannon_offense;
pub mod sustain_offer;

use crate::containers::{SmallList, SortedMap};
use crate::generated::{Breakthrough, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::generated::Agenda;
use crate::kernel::hooks::{CombatCtx, CombatUnit, HookTable};
use crate::keys::atom::Atom;
use crate::keys::{MarkKey, MarkVal};
use crate::query::laws as qlaws;
use crate::state::{CombatDie, Ledger, NoteKey, RerollSet, RollPurpose, RollRec};
use crate::containers::SeatSet;
use crate::generated::Faction;
use crate::kernel::seam::{data as seam_data, Source, Clamp, RowKind, RollKind, RollSite, SeatSystemSite, SilenceCtx, SilenceSite};
use crate::kernel::Kernel;
use crate::query::board as qboard;
use crate::state::{CoreState, Piece, RoundKind};
use crate::tables::Scope;

/// `ti4_model::state::RULES_FACTION_EFFECTS`.
pub const RULES_FACTION_EFFECTS: u32 = 16;
/// `ti4_model::state::RULES_FACTION_BUGFIXES`.
pub const RULES_FACTION_BUGFIXES: u32 = 17;
/// `ti4_model::state::RULES_TECH_EFFECTS`.
pub const RULES_TECH_EFFECTS: u32 = 15;
/// `ti4_model::state::RULES_MUNITIONS_AFTER_ROLL`.
pub const RULES_MUNITIONS_AFTER_ROLL: u32 = 8;
/// `ti4_model::state::RULES_RETREAT_HAZARDS`.
pub const RULES_RETREAT_HAZARDS: u32 = 11;

/// A bound on the round loop (`combat::MAX_ROUNDS`).
pub const MAX_ROUNDS: u32 = 50;

pub use crate::kernel::read::Rd;

/// `neutral_units::is_neutral`: an owner seat that is not a player.
#[must_use]
pub fn is_neutral(s: &CoreState, seat: Seat) -> bool {
    !crate::query::seats::is_player(s, seat)
}

/// Players with ships in a system, in seating order (78.1), then the neutral owner when it has ships.
#[must_use]
pub fn combatants(rd: Rd<'_>, system: System) -> SmallList<Seat, 8> {
    let mut found = SmallList::new();
    for seat in rd.s.seats().seating_order() {
        if !ships_of(rd, *seat, system).is_empty() {
            found.push(*seat);
        }
    }
    // Thunder's Edge neutral units: the neutral owner is never seated.
    let neutral = rd.s.board().sys(system).units.iter().find(|p| is_neutral(rd.s, p.owner()) && rd.is_ship(p.kind()));
    if let Some(p) = neutral {
        found.push(p.owner());
    }
    found
}

/// This player's ships in the system, in board order, then the planet-standing combatants.
#[must_use]
pub fn ships_of(rd: Rd<'_>, player: Seat, system: System) -> Vec<Piece> {
    let mut ships: Vec<Piece> = rd
        .s
        .board()
        .sys(system)
        .units
        .iter()
        .filter(|p| p.owner() == player && rd.is_ship(p.kind()))
        .copied()
        .collect();
    ships.extend(planet_combatants(rd, player, system).into_iter().map(|(_, _, p)| p));
    ships
}

/// The planets whose unit lists live in `system`, in planet (text) order.
#[must_use]
pub fn planets_of_sorted(s: &CoreState, system: System) -> SmallList<Planet, 8> {
    let mut v: SmallList<Planet, 8> = qboard::planets_of(s, system).collect();
    v.as_mut_slice().sort_unstable();
    v
}

/// Units standing on a planet that fight this system's space combat as ships: a Naaz Eidolon Maximum
/// (while its owner has a ship in space) and the ground forces a Nekro Alastor chose.
#[must_use]
pub fn planet_combatants(rd: Rd<'_>, player: Seat, system: System) -> Vec<(Planet, u16, Piece)> {
    let maximum = planetary_maximum_participates(rd, player, system);
    let chosen = rd.k.hooks.seam.alastor_participants.collect(rd.q(), &SeatSystemSite { who: player, system });
    let mut found = Vec::new();
    for planet in planets_of_sorted(rd.s, system).iter() {
        for (index, unit) in rd.s.board().planet(*planet).units.iter().enumerate() {
            if unit.owner() != player {
                continue;
            }
            let voltron = maximum && unit.kind() == Unit::EidolonMaximum;
            if voltron || chosen.iter().any(|(p, i)| p == planet && usize::from(*i) == index) {
                found.push((*planet, index as u16, *unit));
            }
        }
    }
    found
}

/// The corpus clarification for Eidolon Maximum: a planet-standing unit joins its owner's fleet in space
/// combat; cargo alone is not a fleet.
#[must_use]
pub fn planetary_maximum_participates(rd: Rd<'_>, player: Seat, system: System) -> bool {
    rd.s.board().sys(system).units.iter().any(|p| p.owner() == player && rd.is_ship(p.kind()))
}

/// `combat::hits_on`: the value a unit needs to roll, or `None` if it does not fight.
#[must_use]
pub fn hits_on(rd: Rd<'_>, unit: Unit) -> Option<i64> {
    let r = rd.data(unit)?.combat?;
    (r.hits_on > 0).then_some(i64::from(r.hits_on))
}

/// `combat::effective_hits_on`: the threshold a unit needs this round after its round-scoped effects.
#[must_use]
pub fn effective_hits_on(rd: Rd<'_>, player: Seat, unit: Piece) -> Option<i64> {
    effective_from(rd, player, unit, hits_on(rd, unit.kind())?)
}

/// [`effective_hits_on`] for a unit rolling on `threshold` instead of its own printed value: the printed
/// number less the shift every provider adds to the result (`hit_shift`: the Sardakk and Jol-Nar rows,
/// Fighter Prototype, the Ixth law, Morale Boost, the nebula defender, and the faction modules).
#[must_use]
pub fn effective_from(rd: Rd<'_>, player: Seat, unit: Piece, threshold: i64) -> Option<i64> {
    let site = roll_site(rd, player, unit.kind(), rd.s.meta().active_system, 0);
    Some(threshold - rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value)
}

/// The site of a combat roll of `unit` (`system`: the system the providers see; `index`: the unit's place
/// in the fleet list).
#[must_use]
pub fn roll_site(rd: Rd<'_>, owner: Seat, unit: Unit, system: Option<System>, index: u16) -> RollSite {
    RollSite {
        unit: CombatUnit { owner, system, planet: None, unit, ctx: CombatCtx::Space },
        kind: RollKind::Combat,
        round: rd.s.meta().combat_round_seq,
        unit_index: index,
        foes: SeatSet::EMPTY,
    }
}

// -- the fleet's dice -------------------------------------------------------------------------

/// One combat value's pooled fleet dice: how many, and which unit types sit behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetGroup {
    pub dice: i64,
    pub types: SortedMap<Unit, u32, 8>,
}

/// A group's key: the hit number and the roll tag (1: J.N.S. Hylarim's dice, 2: the hulls 0.0.1 binds).
pub type GroupKey = (i64, u8);

/// The flagship texts the combat reads. Local until the generated `Unit -> FlagshipTextSet` lands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlagshipText {
    /// J.N.S. Hylarim: results of 9 or 10 produce 2 additional hits.
    Jolnar,
    /// 0.0.1: hits must be assigned to non-fighter ships if able.
    L1z1x,
    /// Wrath of Kenara: the Hacan flagship.
    Hacan,
}

/// `factions::flagship_has_text`: the unit is the flagship, or a Nekro flagship whose Z token lends it.
pub(crate) fn flagship_has_text(s: &CoreState, who: Seat, unit: Unit, text: FlagshipText) -> bool {
    let (flagship, faction) = match text {
        FlagshipText::Jolnar => (Unit::JNSHylarim, Faction::TheUniversitiesOfJolNar),
        FlagshipText::L1z1x => (Unit::L1z1xFlagship, Faction::TheL1Z1XMindnet),
        FlagshipText::Hacan => (Unit::WrathOfKenara, Faction::TheEmiratesOfHacan),
    };
    if unit == flagship {
        return true;
    }
    unit == Unit::TheAlastor
        && s.players().by_seat(who).is_some_and(|p| {
            p.faction == Faction::TheNekroVirus && p.cards.breakthrough == Some(Breakthrough::ValefarAssimilatorZ)
        })
        && matches!(
            s.marks().tail.get(&MarkKey::NekroToken(who)),
            Some(MarkVal::List(items)) if items.iter().any(|i| matches!(i, MarkVal::Atom(Atom::Faction(f)) if *f == faction))
        )
}

/// Whether `player` has 0.0.1 among its ships in `system`.
#[must_use]
pub fn forces_non_fighters(rd: Rd<'_>, player: Seat, system: System) -> bool {
    ships_of(rd, player, system).iter().any(|p| flagship_has_text(rd.s, player, p.kind(), FlagshipText::L1z1x))
}

/// Dreadnoughts and flagships: the hulls 0.0.1 binds.
#[must_use]
pub fn forced_hull(rd: Rd<'_>, unit: Unit) -> bool {
    rd.data(unit).is_some_and(|d| matches!(d.base_type, UnitBaseType::Flagship | UnitBaseType::Dreadnought))
}

fn roll_tag(rd: Rd<'_>, unit: Unit, jolnar_text: bool, forced: bool) -> u8 {
    if jolnar_text {
        1
    } else if forced && forced_hull(rd, unit) {
        2
    } else {
        0
    }
}

/// The ship that rolls on another card's combat value this combat (The Cavalry): the ship exactly as it
/// stands (type, damage, galvanize), first equal one in board order, and the value it rolls on.
fn borrowed_ship(rd: Rd<'_>, player: Seat, system: System) -> Option<(Piece, i64)> {
    let b = rd.k.hooks.seam.borrowed_stats.offered(rd.q(), &SeatSystemSite { who: player, system })?;
    let value = i64::from(b.hits_on?);
    let piece = rd
        .s
        .board()
        .sys(system)
        .units
        .iter()
        .find(|p| p.owner() == player && p.kind() == b.unit && p.sustained() == b.sustained && p.galvanized() == b.galvanized)?;
    Some((*piece, value))
}

/// The fleet's rolls grouped by (combat value, tag) in the ascending order 78.5b/78.5c fixes.
#[must_use]
pub fn fleet_groups(rd: Rd<'_>, player: Seat, system: System) -> Vec<(GroupKey, FleetGroup)> {
    let s = rd.s;
    let forced = forces_non_fighters(rd, player, system);
    let round = s.meta().combat_round_seq;
    let extra_die = s
        .players()
        .by_seat(player)
        .and_then(|p| if p.combat_timers.extra_die_round.is(round) { p.combat_timers.extra_die_unit } else { None });
    let mut extra_die_added = false;
    // One ship may roll on another card's combat value this combat (The Cavalry).
    let borrower = borrowed_ship(rd, player, system);
    let mut borrower_used = false;
    // (hit number, roll tag, dice, ship type) per ship.
    let mut contributions: Vec<(i64, u8, i64, Unit)> = Vec::new();
    for (unit_index, unit) in ships_of(rd, player, system).into_iter().enumerate() {
        let Some(data) = rd.data(unit.kind()) else { continue };
        let lent = borrower.filter(|(ship, _)| !borrower_used && *ship == unit).map(|(_, v)| v);
        borrower_used |= lent.is_some();
        let Some(value) = (match lent {
            Some(base) => effective_from(rd, player, unit, base),
            None => effective_hits_on(rd, player, unit),
        }) else {
            continue;
        };
        let base_dice = data.combat.map_or(0, |r| i64::from(r.dice));
        let mut site = roll_site(rd, player, unit.kind(), Some(system), unit_index as u16);
        site.unit.system = Some(system);
        let mut dice = rd.k.hooks.seam.dice.eval(rd.q(), &site, base_dice, Clamp::None).value;
        if !extra_die_added && extra_die.is_some_and(|selected| selected == unit.kind()) {
            dice += 1;
            extra_die_added = true;
        }
        let jolnar = flagship_has_text(s, player, unit.kind(), FlagshipText::Jolnar);
        contributions.push((value, roll_tag(rd, unit.kind(), jolnar, forced), dice, unit.kind()));
    }
    gravleash_maneuvers(rd, player, system, &mut contributions);
    let mut groups: Vec<(GroupKey, FleetGroup)> = Vec::new();
    for (value, tag, dice, unit) in contributions {
        let key = (value, tag);
        let at = match groups.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(at) => at,
            Err(at) => {
                groups.insert(at, (key, FleetGroup { dice: 0, types: SortedMap::new() }));
                at
            }
        };
        let group = &mut groups[at].1;
        group.dice += dice;
        *group.types.get_or_insert_with(unit, || 0) += 1;
    }
    groups
}

/// Letnev, Gravleash Maneuvers (rules version 16): +X to one die of one ship, X the ship types in the
/// combat; the die goes where it helps most, the first such ship in board order.
fn gravleash_maneuvers(rd: Rd<'_>, player: Seat, system: System, contributions: &mut Vec<(i64, u8, i64, Unit)>) {
    if rd.s.meta().rules_version < RULES_FACTION_EFFECTS
        || rd.s.players().by_seat(player).and_then(|p| p.cards.breakthrough) != Some(Breakthrough::GravleashManeuvers)
    {
        return;
    }
    let mut kinds: Vec<Unit> = ships_of(rd, player, system).into_iter().map(|p| p.kind()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let bonus = kinds.len() as i64;
    let chance = |hit_on: i64| (11 - hit_on).clamp(0, 10);
    let mut best: Option<(usize, i64)> = None;
    for (index, (value, _, dice, _)) in contributions.iter().enumerate() {
        if *dice <= 0 {
            continue;
        }
        let gain = chance((value - bonus).max(1)) - chance(*value);
        if best.is_none_or(|(_, top)| gain > top) {
            best = Some((index, gain));
        }
    }
    let Some((index, gain)) = best else { return };
    if bonus <= 0 || gain <= 0 {
        return;
    }
    let (value, tag, dice, unit) = contributions[index];
    if dice > 1 {
        contributions[index].2 = dice - 1;
        contributions.push(((value - bonus).max(1), tag, 1, unit));
    } else {
        contributions[index].0 = (value - bonus).max(1);
    }
}

/// One recorded roll of the fleet, in `Dice` history order: the hit number and the faces. A munitions
/// reroll is its own entry, beside the roll it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRoll {
    pub hits_on: u32,
    pub faces: Vec<u32>,
    /// The dice a munitions reroll replaced (empty for the roll itself).
    pub rerolled: Vec<u32>,
}

/// The ten-sided die of `dice::SIDES`.
pub const DIE_SIDES: u32 = 10;

/// Roll one player's fleet (78.5), group by group in ascending combat value, drawing from `rng`'s dice
/// stream exactly as `combat::roll_fleet` does; returns the history entries it would record (the
/// roll of each group and, when Munitions Reserves is active this round, the reroll of its misses).
pub fn roll_fleet(
    rd: Rd<'_>,
    rng: &mut crate::state::Rng,
    seeds: &dyn crate::state::SeedSource,
    player: Seat,
    system: System,
) -> Vec<FleetRoll> {
    use crate::state::RngDomain;
    let round = rd.s.meta().combat_round_seq;
    let munitions = rd.s.players().by_seat(player).is_some_and(|p| p.combat_timers.munitions_round.is(round));
    let mut out = Vec::new();
    for ((value, _), group) in fleet_groups(rd, player, system) {
        let count = usize::try_from(group.dice).unwrap_or(0);
        if count == 0 {
            continue;
        }
        let hits_on = u32::try_from(value).unwrap_or(u32::MAX);
        let faces: Vec<u32> = (0..count).map(|_| rng.die(RngDomain::Dice, seeds, DIE_SIDES)).collect();
        let misses: Vec<usize> = faces.iter().enumerate().filter(|(_, f)| **f < hits_on).map(|(i, _)| i).collect();
        out.push(FleetRoll { hits_on, faces: faces.clone(), rerolled: Vec::new() });
        if munitions && !misses.is_empty() {
            let mut again = faces;
            let rerolled = misses.iter().map(|i| *i as u32).collect();
            for i in misses {
                again[i] = rng.die(RngDomain::Dice, seeds, DIE_SIDES);
            }
            out.push(FleetRoll { hits_on, faces: again, rerolled });
        }
    }
    out
}

// -- sustain damage and hit assignment (LRR 87, 78.4) -------------------------------------------

/// `laws::sustain_suppressed`: Publicize Weapon Schematics takes SUSTAIN DAMAGE from all war suns.
#[must_use]
pub fn sustain_suppressed(s: &CoreState, blanked: bool, base: UnitBaseType) -> bool {
    base == UnitBaseType::Warsun && qlaws::active(s, blanked, Agenda::PublicizeWeaponSchematics)
}

/// A ship of its own accord: a ship with SUSTAIN DAMAGE, or any non-fighter ship under Metali Void
/// Shielding.
fn sustains_by_itself(rd: Rd<'_>, seat: Seat, unit: Unit) -> bool {
    rd.is_ship(unit)
        && (rd.data(unit).is_some_and(|d| d.sustain_damage) || (seam_data::grants_sustain(rd.q(), &rd.k.hooks.seam.rows, seat) && !rd.is_fighter(unit)))
}

/// `combat::sustains_in_space`: whether this unit may use SUSTAIN DAMAGE against a hit in space.
/// `in_combat` is whether the hit is part of a space combat rather than a SPACE CANNON OFFENSE hit.
#[must_use]
pub fn sustains_in_space(rd: Rd<'_>, system: System, player: Seat, unit: Piece, in_combat: bool) -> bool {
    let Some(data) = rd.data(unit.kind()) else { return false };
    let cu = |ctx| CombatUnit { owner: player, system: Some(system), planet: None, unit: unit.kind(), ctx };
    unit.owner() == player
        && !unit.sustained()
        && !sustain_suppressed(rd.s, rd.blanked, data.base_type)
        && (sustains_by_itself(rd, player, unit.kind())
            // The string context "space_cannon" (a SPACE CANNON OFFENSE hit) has no `CombatCtx`: no
            // module grants the ability there, so the slot is asked for combat hits only.
            || (in_combat && rd.k.hooks.combat.grants_sustain(rd.s, rd.k, cu(CombatCtx::Space))))
        && rd.k.hooks.seam.may_sustain.decide(rd.q(), &cu(CombatCtx::Space), true)
}

/// `combat::assignable_to_hit`: a hit of a unit ability cannot be assigned to a unit that is immune to
/// them (Naaz Eidolon Maximum).
#[must_use]
pub fn assignable_to_hit(rd: Rd<'_>, player: Seat, system: System, unit: Unit, planet: Option<Planet>, ability: bool) -> bool {
    !ability
        || !rd.k.hooks.combat.ability_hit_immune(
            rd.s,
            rd.k,
            CombatUnit { owner: player, system: Some(system), planet, unit, ctx: CombatCtx::Space },
        )
}

/// `combat::sustainer_counts`: how many sustains of each unit type the seat can use in a row, in
/// unit-type order. `available` is every candidate as (stands on a planet, unit).
#[must_use]
pub fn sustainer_counts(
    rd: Rd<'_>,
    system: System,
    player: Seat,
    in_combat: bool,
    available: &[(bool, Piece)],
) -> Vec<(Unit, u64)> {
    let mut counts: Vec<(Unit, u64)> = Vec::new();
    for (_, piece) in available {
        match counts.iter_mut().find(|(u, _)| *u == piece.kind()) {
            Some((_, n)) => *n += 1,
            None => counts.push((piece.kind(), 1)),
        }
    }
    for (kind, count) in &mut counts {
        if rd.data(*kind).is_none() {
            continue;
        }
        let in_space = available.iter().filter(|(_, p)| p.kind() == *kind).all(|(planet, _)| !planet);
        if *count < 2 || sustains_by_itself(rd, player, *kind) || !in_space {
            continue;
        }
        let mut played = rd.s.clone();
        let mut used = 0_u64;
        loop {
            let at = played.board().sys(system).units.iter().position(|p| {
                p.kind() == *kind && sustains_in_space(rd.over(&played), system, player, *p, in_combat)
            });
            let Some(at) = at else { break };
            let cell = played.board_mut().sys_mut(system);
            cell.units.as_mut_slice()[at] = cell.units.as_slice()[at].with_sustained(true);
            used += 1;
        }
        *count = used.max(1);
    }
    counts
}

/// `combat::direct_hittable`: the printed rule (a unit the corpus does not know is hittable).
#[must_use]
pub fn direct_hittable(scope: Scope, unit: Unit) -> bool {
    scope.unit(unit).is_none_or(|d| d.can_be_direct_hit)
}

/// `combat::direct_hittable_at`: the printed rule and no module's immunity.
#[must_use]
pub fn direct_hittable_at(rd: Rd<'_>, player: Seat, system: System, unit: Unit) -> bool {
    direct_hittable(rd.scope(), unit)
        && !rd.k.hooks.seam.hit_immune.decide(
            rd.q(),
            &CombatUnit { owner: player, system: Some(system), planet: None, unit, ctx: CombatCtx::Space },
            false,
        )
}

// -- the Naaz and Nomad hooks -------------------------------------------------------------------

/// Naaz: the Eidolon Maximum cannot be assigned hits from unit abilities (`naaz::ability_hit_immune`).
#[must_use]
pub fn naaz_ability_hit_immune(_: &CoreState, _: &Kernel, unit: CombatUnit) -> bool {
    unit.unit == Unit::EidolonMaximum
}

/// The ship type `holder` chose for The Cavalry in the combat in `system`, while the note is face up
/// with them (`nomad_agents::cavalry_ship_type`).
fn cavalry_ship_type(s: &CoreState, holder: Seat, system: System) -> Option<Unit> {
    let MarkVal::Rec(fields) = s.marks().tail.get(&MarkKey::NomadCavalry(holder))? else { return None };
    let [MarkVal::Atom(Atom::System(marked)), MarkVal::Atom(Atom::Unit(kind))] = fields.as_slice() else { return None };
    let note = NoteKey { note: crate::generated::PromissoryNote::TheCavalry, faction: crate::generated::Faction::TheNomad };
    let cards = s.cards();
    (*marked == system && cards.promissory.get(&note) == Some(&holder) && cards.promissory_faceup.contains_key(&note))
        .then_some(*kind)
}

/// Nomad: Quantum Manipulator and The Cavalry give SUSTAIN DAMAGE against a hit in a space combat
/// (`nomad_agents::grants_sustain`).
#[must_use]
pub fn nomad_grants_sustain(s: &CoreState, _: &Kernel, unit: CombatUnit) -> bool {
    if unit.ctx != CombatCtx::Space {
        return false;
    }
    if unit.unit == Unit::QuantumManipulator {
        return true;
    }
    let Some(system) = unit.system else { return false };
    cavalry_ship_type(s, unit.owner, system) == Some(unit.unit)
        && !s.board().sys(system).units.iter().any(|p| p.owner() == unit.owner && p.kind() == unit.unit && p.sustained())
}

/// Fill the combat slots the core carries for Naaz and Nomad (the C shards register the rest).
pub fn register_naaz_nomad(t: &mut HookTable) {
    t.combat.ability_hit_immune.push(naaz_ability_hit_immune);
    t.combat.grants_sustain.push(nomad_grants_sustain);
}

// -- the dice history and the round's dice ------------------------------------------------------

/// Append the fleet's rolls to the dice history (`Dice::history`): the roll of each group is a
/// space-combat entry, a munitions reroll an entry of purpose [`RollPurpose::Reroll`] beside it. Every
/// entry names the seat whose dice they are and the scene system the host stamps on it.
pub fn record_fleet_rolls(ledger: &mut Ledger, owner: Seat, scene: Option<System>, rolls: &[FleetRoll]) {
    record_rolls(ledger, owner, scene, rolls, RollPurpose::SpaceCombat);
}

/// [`record_fleet_rolls`] for rolls of another purpose; a roll with `rerolled` dice is a `Reroll` entry.
pub fn record_rolls(ledger: &mut Ledger, owner: Seat, scene: Option<System>, rolls: &[FleetRoll], purpose: RollPurpose) {
    for r in rolls {
        let mask = r.rerolled.iter().filter(|i| **i < 64).fold(0_u64, |m, i| m | (1 << i));
        ledger.dice.push(RollRec {
            purpose: if r.rerolled.is_empty() { purpose } else { RollPurpose::Reroll },
            owner: Some(owner),
            unit: None,
            planet: None,
            scene,
            hits_on: Some(u8::try_from(r.hits_on).unwrap_or(u8::MAX)),
            faces: r.faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
            rerolled: mask,
        });
    }
}

/// The hits a group of rolls scored: the faces of its last entry at or above the hit number.
#[must_use]
pub fn hits_of(roll: &FleetRoll) -> usize {
    roll.faces.iter().filter(|f| **f >= roll.hits_on).count()
}

/// `combat_round_dice`: one record per staged die of the round, the attacker's sets first. A die's
/// bonus (Wrath of Kenara and its kind) counts towards the hit, the printed face is what is recorded.
#[must_use]
pub fn round_dice(sides: &[(Seat, Option<&RerollSet>)]) -> Vec<CombatDie> {
    let mut out = Vec::new();
    for (side, set) in sides {
        let Some(set) = set else { continue };
        for entry in &set.rolls {
            let target = entry.hits_on.map_or(0, u32::from);
            for (i, face) in entry.faces.iter().enumerate() {
                let delta = u8::try_from(i).ok().and_then(|i| entry.deltas.get(&i).copied()).unwrap_or(0);
                let value = (i64::from(*face) + i64::from(delta)).max(0) as u32;
                out.push(CombatDie { player: *side, unit: entry.unit, roll: u32::from(*face), target, hit: target > 0 && value >= target });
            }
        }
    }
    out
}

// -- the offer to sustain (LRR 87.4) -------------------------------------------------------------

use crate::choice::combat::{Ask as CombatAsk, Opt as CombatOpt, SustainSite};
use crate::choice::{Opt, Opts};

/// `combat::planet_unit_sustains`: whether a unit standing on a planet may sustain against a hit in
/// space. A ship follows the space rule; any other unit needs its own printed ability.
#[must_use]
pub fn planet_unit_sustains(rd: Rd<'_>, system: System, player: Seat, unit: Piece, in_combat: bool) -> bool {
    let Some(data) = rd.data(unit.kind()) else { return false };
    if data.ship {
        return sustains_in_space(rd, system, player, unit, in_combat);
    }
    unit.owner() == player
        && !unit.sustained()
        && data.sustain_damage
        && !sustain_suppressed(rd.s, rd.blanked, data.base_type)
        && rd.k.hooks.seam.may_sustain.decide(
            rd.q(),
            &CombatUnit { owner: player, system: Some(system), planet: None, unit: unit.kind(), ctx: CombatCtx::Space },
            true,
        )
}

/// Every unit that could take the sustain of a hit, in the order the question lists them: space
/// units in board order, then the planet-standing combatants.
#[must_use]
pub fn sustain_candidates(
    rd: Rd<'_>,
    player: Seat,
    system: System,
    in_combat: bool,
    ability: bool,
    staging: bool,
) -> Vec<(SustainSite, Piece)> {
    let mut found = Vec::new();
    for (index, unit) in rd.s.board().sys(system).units.iter().enumerate() {
        if rd.data(unit.kind()).is_some()
            && sustains_in_space(rd, system, player, *unit, in_combat)
            && assignable_to_hit(rd, player, system, unit.kind(), None, ability)
        {
            found.push((SustainSite::Space(index as u16), *unit));
        }
    }
    let mut seen: Vec<(Planet, Piece)> = Vec::new();
    for (planet, index, unit) in planet_combatants(rd, player, system) {
        if !planet_unit_sustains(rd, system, player, unit, in_combat)
            || !assignable_to_hit(rd, player, system, unit.kind(), Some(planet), ability)
        {
            continue;
        }
        if seen.iter().any(|(p, u)| *p == planet && *u == unit) {
            continue;
        }
        let first_here = !seen.iter().any(|(p, _)| *p == planet);
        let variant = unit.kind() != Unit::EidolonMaximum || (staging && !first_here);
        seen.push((planet, unit));
        found.push((SustainSite::Planet { planet, index, variant }, unit));
    }
    found
}

/// The options of [`CombatAsk::CancelHit`]: one per unit type (every planet-standing unit under staging),
/// then the generic decline ("take the hit"). Empty when nothing can sustain.
#[must_use]
pub fn sustain_opts(rd: Rd<'_>, player: Seat, ask: CombatAsk) -> Opts {
    let CombatAsk::CancelHit { system, staging, ability, in_combat, .. } = ask else { return Opts::default() };
    let mut opts = Opts::default();
    let mut types: Vec<Unit> = Vec::new();
    let found = sustain_candidates(rd, player, system, in_combat, ability, staging);
    if found.is_empty() {
        return opts;
    }
    for (site, unit) in found {
        let on_planet = matches!(site, SustainSite::Planet { .. });
        if !(staging && on_planet) {
            if types.contains(&unit.kind()) {
                continue;
            }
            types.push(unit.kind());
        }
        opts.push(Opt::Combat(CombatOpt::Sustain { site, unit: unit.kind(), galvanized: unit.galvanized() }));
    }
    opts.push(Opt::Decline);
    opts
}

// -- assigning a hit (LRR 78.6) and using a sustain ----------------------------------------------

/// `combat::choose_casualty_owing`: one option per distinguishable loss -- the first unit of each
/// (type, damaged) pair -- in list order.
#[must_use]
pub fn casualty_opts(units: &[Piece]) -> Opts {
    let mut opts = Opts::default();
    let mut seen: Vec<(Unit, bool)> = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        let key = (unit.kind(), unit.sustained());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        opts.push(Opt::Combat(CombatOpt::Destroy { index: index as u16, unit: unit.kind(), damaged: unit.sustained() }));
    }
    opts
}

/// How many hits one use of SUSTAIN DAMAGE cancels: 2 with Non-Euclidean Shielding, else 1.
#[must_use]
pub fn cancel_per_sustain(rd: Rd<'_>, player: Seat) -> usize {
    match seam_data::cancel_on_sustain(rd.q(), &rd.k.hooks.seam.rows, player) {
        n if n > 0 => n as usize,
        _ => 1,
    }
}

/// The state after `player` sustains at `site`: the unit is marked damaged. `None` when no such unit
/// can (the string leaves the position alone then). A planet unit named by its index must be one of
/// the combatants and undamaged; one named by its planet is the owner's first undamaged Maximum there.
#[must_use]
pub fn apply_sustain(rd: Rd<'_>, player: Seat, system: System, site: SustainSite) -> Option<(CoreState, Piece)> {
    let mut after = rd.s.clone();
    let done;
    match site {
        SustainSite::Space(i) => {
            let cell = after.board_mut().sys_mut(system);
            let slot = cell.units.as_mut_slice().get_mut(usize::from(i))?;
            *slot = slot.with_sustained(true);
            done = *slot;
        }
        SustainSite::Planet { planet, index, variant } => {
            let combatants = planet_combatants(rd, player, system);
            let units = &mut after.board_mut().planet_mut(planet).units;
            let slot = if variant {
                let at = usize::from(index);
                units.as_mut_slice().get_mut(at).filter(|u| {
                    u.owner() == player && combatants.iter().any(|(p, i, _)| *p == planet && usize::from(*i) == at) && !u.sustained()
                })
            } else {
                units.as_mut_slice().iter_mut().find(|u| u.owner() == player && u.kind() == Unit::EidolonMaximum && !u.sustained())
            };
            let slot = slot?;
            *slot = slot.with_sustained(true);
            done = *slot;
        }
    }
    Some((after, done))
}

// -- anti-fighter barrage (LRR 78.3) -------------------------------------------------------------

/// One barrage roll a side makes: the ship (`None`: the relic, which fires once for its holder), the
/// number the dice need and how many dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarrageRoll {
    pub unit: Option<Unit>,
    pub hits_on: i64,
    pub dice: usize,
}

/// Iconoclast Omega: "Other players cannot use ANTI-FIGHTER BARRAGE against your units in this system"
/// (rules version 17), unless the mech's own abilities are lost.
fn omega_forbids_barrage(rd: Rd<'_>, system: System, player: Seat) -> bool {
    if rd.s.meta().rules_version < RULES_FACTION_BUGFIXES {
        return false;
    }
    let board = rd.s.board();
    let in_space = board.sys(system).units.iter().copied();
    let planets = planets_of_sorted(rd.s, system);
    let on_planets = planets.iter().flat_map(|p| board.planet(*p).units.iter().copied().collect::<Vec<_>>());
    in_space.chain(on_planets).any(|u| {
        u.owner() != player
            && u.kind() == Unit::IconoclastOmega
            && !rd.k.hooks.seam.silenced.decide(
                rd.q(),
                &SilenceSite { owner: u.owner(), system, unit: None, ctx: SilenceCtx::Space },
                false,
            )
    })
}

/// The shift fighters' own rows add to the roll (Fighter Prototype, the Ixth law), the part of
/// `hit_shift` a barrage reads besides its own site.
fn fighter_row_shift(rd: Rd<'_>, player: Seat) -> i64 {
    let round = rd.s.meta().combat_round_seq;
    seam_data::COMBAT_ROWS
        .iter()
        .filter(|r| r.fighters_only && matches!(r.kind, RowKind::RollShift(_)))
        .map(|r| r.amount(r.times(rd.q(), player, SeatSet::EMPTY, None, round)))
        .sum()
}

/// `combat::roll_barrage_side`'s plan: the rolls `player`'s side makes in `system`, in order.
#[must_use]
pub fn barrage_plan(rd: Rd<'_>, player: Seat, system: System) -> Vec<BarrageRoll> {
    let mut plan = Vec::new();
    if omega_forbids_barrage(rd, system, player) {
        return plan;
    }
    // Metali Void Armaments fires once for its holder.
    if let Some(RowKind::Barrage { hits_on, dice }) = rd
        .k
        .hooks
        .seam
        .rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Barrage { .. }) && r.times(rd.q(), player, SeatSet::EMPTY, None, 0) > 0)
        .map(|r| r.kind)
        .next()
    {
        plan.push(BarrageRoll { unit: None, hits_on: i64::from(hits_on), dice: usize::from(dice) });
    }
    let borrower = rd
        .k
        .hooks
        .seam
        .borrowed_stats
        .offered(rd.q(), &SeatSystemSite { who: player, system })
        .and_then(|b| b.barrage.map(|(v, d)| (b.unit, b.sustained, b.galvanized, i64::from(v), i64::from(d))));
    let mut borrower_used = false;
    let lost = rd.k.hooks.seam.silenced.decide(
        rd.q(),
        &SilenceSite { owner: player, system, unit: None, ctx: SilenceCtx::Space },
        false,
    );
    for (index, unit) in ships_of(rd, player, system).into_iter().enumerate() {
        let Some(data) = rd.data(unit.kind()) else { continue };
        if lost {
            continue;
        }
        let lent = borrower
            .filter(|(u, s, g, ..)| !borrower_used && *u == unit.kind() && *s == unit.sustained() && *g == unit.galvanized())
            .map(|(.., v, d)| (v, d));
        borrower_used |= lent.is_some();
        let Some(printed) = lent.map(|(v, _)| v).or_else(|| data.anti_fighter_barrage.map(|r| i64::from(r.hits_on))) else {
            continue;
        };
        let count = lent.map_or_else(|| data.anti_fighter_barrage.map_or(0, |r| i64::from(r.dice)), |(_, d)| d);
        if count <= 0 {
            continue;
        }
        let mut site = roll_site(rd, player, unit.kind(), Some(system), index as u16);
        site.kind = RollKind::Barrage;
        let dice = rd.k.hooks.seam.dice.eval(rd.q(), &site, count, Clamp::None).value;
        let mut value = printed;
        if rd.is_fighter(unit.kind()) {
            value -= fighter_row_shift(rd, player);
        }
        value = (value - rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value).max(1);
        plan.push(BarrageRoll { unit: Some(unit.kind()), hits_on: value, dice: usize::try_from(dice).unwrap_or(0) });
    }
    plan
}

// -- space cannon (LRR 77) -----------------------------------------------------------------------

/// One gun's roll in the space cannon offense: its owner, the unit and how it rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CannonRoll {
    pub owner: Seat,
    pub unit: Unit,
    pub hits_on: i64,
    pub dice: usize,
}

/// Whether `owner` has Plasma Scoring (the data row of the technology).
fn has_plasma_scoring(rd: Rd<'_>, owner: Seat) -> bool {
    seam_data::COMBAT_ROWS
        .iter()
        .filter(|r| matches!(r.kind, RowKind::ExtraDice(_)) && r.source == Source::Technology(crate::generated::Technology::PlasmaScoring))
        .any(|r| r.times(rd.q(), owner, SeatSet::EMPTY, None, 0) > 0)
}

/// `combat::space_cannon_offense`'s per-gun math for the guns in `guns` (owner, unit, in firing order):
/// Disable, Plasma Scoring's pick, the Bastion die, Assail (clamped at 1) and then Antimass Deflectors.
/// `opponent` is the player the active player's guns fire at. Guns without a SPACE CANNON, or with no
/// dice, are skipped.
#[must_use]
pub fn cannon_plan(rd: Rd<'_>, system: System, active: Seat, opponent: Option<Seat>, guns: &[(Seat, Unit)]) -> Vec<CannonRoll> {
    let activation = rd.s.meta().activation_seq;
    // Plasma Scoring: one of a player's firing units rolls a die more, the one that hits most easily.
    let mut picks: Vec<(Seat, i64)> = Vec::new();
    for (owner, unit) in guns {
        let Some(value) = rd.data(*unit).and_then(|d| d.space_cannon).map(|r| i64::from(r.hits_on)).filter(|v| *v > 0) else { continue };
        if !has_plasma_scoring(rd, *owner) {
            continue;
        }
        match picks.iter_mut().find(|(s, _)| s == owner) {
            Some((_, best)) => *best = (*best).min(value),
            None => picks.push((*owner, value)),
        }
    }
    let mut out = Vec::new();
    for (owner, unit) in guns {
        let Some(data) = rd.data(*unit) else { continue };
        let Some(printed) = data.space_cannon.map(|r| i64::from(r.hits_on)).filter(|v| *v > 0) else { continue };
        if data.base_type == UnitBaseType::Pds
            && rd
                .s
                .players()
                .iter()
                .any(|p| p.seat != *owner && p.round_marks.contains(RoundKind::DisableInvasion, activation))
        {
            continue;
        }
        let dice = data.space_cannon.map_or(0, |r| i64::from(r.dice));
        if dice <= 0 {
            continue;
        }
        let plasma = match picks.iter().position(|(s, v)| s == owner && *v == printed) {
            Some(at) => {
                picks.remove(at);
                1
            }
            None => 0,
        };
        // The Bastion die: the barrage site carries it and no row of the cannon.
        let mut site = roll_site(rd, *owner, *unit, Some(system), 0);
        site.kind = RollKind::Barrage;
        let dice = rd.k.hooks.seam.dice.eval(rd.q(), &site, dice + plasma, Clamp::None).value;
        let target = if *owner == active { opponent } else { Some(active) };
        let mut foes = SeatSet::EMPTY;
        if let Some(t) = target {
            foes.insert(t);
        }
        site.foes = foes;
        site.kind = RollKind::CannonAssail;
        let assail = rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value;
        let mut value = (printed - assail).max(1);
        if rd.s.meta().rules_version >= RULES_TECH_EFFECTS && target.is_some_and(|t| t != *owner) {
            site.kind = RollKind::SpaceCannon;
            value -= rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value;
        }
        out.push(CannonRoll { owner: *owner, unit: *unit, hits_on: value, dice: usize::try_from(dice).unwrap_or(0) });
    }
    out
}

/// Roll one side's anti-fighter barrage from `rng`'s dice stream, entry by entry of [`barrage_plan`], as
/// `combat::roll_barrage_side` does; returns the history entries it records.
pub fn roll_barrage(
    rd: Rd<'_>,
    rng: &mut crate::state::Rng,
    seeds: &dyn crate::state::SeedSource,
    player: Seat,
    system: System,
) -> Vec<FleetRoll> {
    use crate::state::RngDomain;
    barrage_plan(rd, player, system)
        .into_iter()
        .map(|r| FleetRoll {
            hits_on: u32::try_from(r.hits_on).unwrap_or(u32::MAX),
            faces: (0..r.dice).map(|_| rng.die(RngDomain::Dice, seeds, DIE_SIDES)).collect(),
            rerolled: Vec::new(),
        })
        .collect()
}

// -- retreat (LRR 78.7) --------------------------------------------------------------------------

/// `combat::retreat_may_enter`: whether a retreat may end in `destination` under the movement rules.
/// Games below `RULES_RETREAT_HAZARDS` never checked. `rules_for` gives the mover's rules for moving into
/// a system (`SeatReach::rules_for`).
#[must_use]
pub fn retreat_may_enter(s: &CoreState, destination: System, rules_for: &dyn Fn(System) -> crate::movement::CoreRules) -> bool {
    s.meta().rules_version < RULES_RETREAT_HAZARDS || rules_for(destination).can_enter(destination)
}

/// `combat::eligible_retreats` (78.7c): adjacent systems (as the mover sees the map) that hold the mover's
/// units or a planet it controls and no other player's ships, that the movement rules let it enter.
#[must_use]
pub fn eligible_retreats(
    rd: Rd<'_>,
    seat: Seat,
    system: System,
    adjacency: &crate::galaxy::Adjacency,
    rules_for: &dyn Fn(System) -> crate::movement::CoreRules,
) -> Vec<System> {
    adjacency
        .of(system)
        .iter()
        .map(crate::ids::system_at)
        .filter(|n| !adjacency.is_hyperlane(*n))
        .filter(|n| retreat_may_enter(rd.s, *n, rules_for))
        .filter(|n| {
            let cell = rd.s.board().sys(*n);
            if cell.units.iter().any(|u| u.owner() != seat && rd.is_ship(u.kind())) {
                return false;
            }
            cell.units.iter().any(|u| u.owner() == seat) || qboard::controls_a_planet_in(rd.s, *n, seat)
        })
        .collect()
}

// -- who a hit may be assigned to (LRR 78.6, 15.2a) ----------------------------------------------

/// The units `player` may assign one more hit to, in the order the casualty question lists them: the
/// ships (and planet-standing combatants) that can be assigned a hit of this origin, narrowed to the
/// non-fighters when the loss is bound to them and one is able. Empty: the hit has no effect.
#[must_use]
pub fn hit_targets(rd: Rd<'_>, player: Seat, system: System, ability: bool, non_fighters_first: bool) -> Vec<Piece> {
    let board = rd.s.board();
    let space = &board.sys(system).units;
    let planets = planets_of_sorted(rd.s, system);
    let mut alive: Vec<Piece> = ships_of(rd, player, system)
        .into_iter()
        .filter(|unit| {
            let planet = if space.contains(unit) {
                None
            } else {
                planets.iter().find(|p| board.planet(**p).units.contains(unit)).copied()
            };
            assignable_to_hit(rd, player, system, unit.kind(), planet, ability)
        })
        .collect();
    if non_fighters_first {
        let bound: Vec<Piece> = ships_of(rd, player, system)
            .into_iter()
            .filter(|p| !rd.is_fighter(p.kind()))
            .filter(|p| alive.contains(p))
            .collect();
        if !bound.is_empty() {
            alive = bound;
        }
    }
    alive
}

/// Where Skilled Retreat (`foreign_units: false`: no other player's ships) or Dark Energy Tap
/// (`foreign_units: true`: no other player's units, ships or ground forces) may send a fleet: the
/// adjacent systems of the plain map the movement rules let it enter. The caller handles the Mahact hero
/// bar, which is string-only.
#[must_use]
pub fn open_retreat_destinations(
    rd: Rd<'_>,
    seat: Seat,
    system: System,
    adjacency: &crate::galaxy::Adjacency,
    rules_for: &dyn Fn(System) -> crate::movement::CoreRules,
    foreign_units: bool,
) -> Vec<System> {
    adjacency
        .of(system)
        .iter()
        .map(crate::ids::system_at)
        .filter(|n| !adjacency.is_hyperlane(*n))
        .filter(|n| retreat_may_enter(rd.s, *n, rules_for))
        .filter(|n| {
            let cell = rd.s.board().sys(*n);
            if foreign_units {
                !cell.units.iter().any(|u| u.owner() != seat)
                    && !planets_of_sorted(rd.s, *n).iter().any(|p| rd.s.board().planet(*p).units.iter().any(|u| u.owner() != seat))
            } else {
                !cell.units.iter().any(|u| u.owner() != seat && rd.is_ship(u.kind()))
            }
        })
        .collect()
}
