//! Space combat (LRR 78, with 87 governing Sustain Damage): the typed port of `ti4-engine/src/combat.rs`
//! (package B1).
//!
//! Layout of this file, in the order of the string module:
//!
//! * the **faction seam** ([`Faction`]): every call the string combat makes into faction code that has no
//!   core implementation yet. The default implementation, [`KernelFaction`], forwards to the kernel's
//!   [`HookTable`](crate::kernel::HookTable); the dual harness passes the string engine's answers. The
//!   list of these calls is the list of string-only dependencies of the area.
//! * queries over the board: [`combatants`], [`ships_of`], [`hits_on`], [`effective_hits_on`];
//! * the fleet's dice: [`fleet_groups`], [`roll_fleet`];
//! * retreats, cancellations, anti-fighter barrage and space cannon follow in the later sections.
//!
//! Nothing here asks a question or emits an event: those go through the host the later sections take.

use crate::containers::{SmallList, SortedMap};
use crate::generated::{Breakthrough, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::generated::{Agenda, Relic};
use crate::kernel::hooks::{CombatCtx, CombatUnit, HookTable};
use crate::keys::atom::Atom;
use crate::keys::{MarkKey, MarkVal};
use crate::query::laws as qlaws;
use crate::state::{CombatDie, Ledger, NoteKey, RerollSet, RollPurpose, RollRec};
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

/// `hooks_combat::BorrowedStats`: a ship that rolls on another card's stats (The Cavalry). `unit` is the
/// ship as it stands (the first equal one in board order is the borrower).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borrowed {
    pub unit: Piece,
    pub hits_on: Option<i64>,
    /// ANTI-FIGHTER BARRAGE as `(value, dice)`.
    pub barrage: Option<(i64, i64)>,
}

/// The flagship texts the combat reads (`factions::flagship_has_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagshipText {
    /// J.N.S. Hylarim: results of 9 or 10 produce 2 additional hits.
    Jolnar,
    /// Arc Secundus: repair at the start of each space combat round.
    Letnev,
    /// 0.0.1: hits must be assigned to non-fighter ships if able.
    L1z1x,
    /// Wrath of Kenara.
    Hacan,
}

/// Every call the space combat makes into faction code. One method per string function, all reading
/// the state `s` the core is working on.
pub trait Faction {
    /// `nekro_units::alastor_participants`: ground forces chosen to fight as ships, as (planet, index
    /// in that planet's unit list).
    fn alastor_participants(&self, s: &CoreState, who: Seat, system: System) -> SmallList<(Planet, u16), 4>;
    /// `factions::flagship_has_text`.
    fn flagship_has_text(&self, s: &CoreState, who: Seat, unit: Unit, text: FlagshipText) -> bool;
    /// `laws::fighter_combat_bonus`.
    fn law_fighter_bonus(&self, s: &CoreState, who: Seat) -> i64;
    /// `faction_abilities::combat_modifier`.
    fn combat_modifier(&self, s: &CoreState, who: Seat, ctx: CombatCtx) -> i64;
    /// `factions::unit_roll_modifier`.
    fn unit_roll_modifier(&self, s: &CoreState, unit: CombatUnit) -> i64;
    /// `mahact_units::flagship_roll_bonus`.
    fn mahact_flagship_bonus(&self, s: &CoreState, who: Seat, unit: Unit) -> i64;
    /// `nekro_units::mech_roll_bonus`.
    fn nekro_mech_bonus(&self, s: &CoreState, who: Seat, unit: Unit, system: Option<System>) -> i64;
    /// `firmament_plots::assail_space`.
    fn assail_space(&self, s: &CoreState, who: Seat, system: Option<System>) -> i64;
    /// `factions::unit_dice`: the unit's dice given the count so far.
    fn unit_dice(&self, s: &CoreState, unit: CombatUnit, dice: i64) -> i64;
    /// `hooks_combat::borrowed_stats`.
    fn borrowed_stats(&self, s: &CoreState, who: Seat, system: System) -> Option<Borrowed>;
    /// `hooks_combat::may_sustain` for the modules the core does not carry (Crimson's breach rule,
    /// Mentak's flagship and mech); true when none forbids it.
    fn may_sustain_modules(&self, s: &CoreState, unit: CombatUnit) -> bool;
    /// `bastion::extra_die`: a galvanized unit rolls one more die.
    fn bastion_extra_die(&self, s: &CoreState, piece: Piece) -> i64;
    /// `borrowed_round_agents::extra_die_for`.
    fn round_agent_extra_die(&self, s: &CoreState, system: System, who: Seat, unit: Unit, index: usize) -> i64;
}

/// The faction seam answered by the kernel's hook table (what the driver uses once the faction modules
/// are registered). `flagship_has_text`, `law_fighter_bonus` and `combat_modifier` have no hook slot of
/// their own yet and answer neutral.
pub struct KernelFaction<'a>(pub &'a Kernel);

impl Faction for KernelFaction<'_> {
    fn alastor_participants(&self, s: &CoreState, who: Seat, system: System) -> SmallList<(Planet, u16), 4> {
        self.0.hooks.direct.nekro_units_alastor_participants(s, who, system)
    }
    fn flagship_has_text(&self, _: &CoreState, _: Seat, _: Unit, _: FlagshipText) -> bool {
        false
    }
    fn law_fighter_bonus(&self, _: &CoreState, _: Seat) -> i64 {
        0
    }
    fn combat_modifier(&self, s: &CoreState, who: Seat, ctx: CombatCtx) -> i64 {
        self.0.hooks.general.combat_modifier(s, self.0, who, ctx)
    }
    fn unit_roll_modifier(&self, s: &CoreState, unit: CombatUnit) -> i64 {
        self.0.hooks.general.unit_roll_modifier(s, self.0, unit)
    }
    fn mahact_flagship_bonus(&self, s: &CoreState, who: Seat, unit: Unit) -> i64 {
        self.0.hooks.direct.mahact_units_flagship_roll_bonus(s, self.0, who, unit)
    }
    fn nekro_mech_bonus(&self, s: &CoreState, who: Seat, unit: Unit, system: Option<System>) -> i64 {
        self.0.hooks.direct.nekro_units_mech_roll_bonus(s, self.0, who, unit, system, None)
    }
    fn assail_space(&self, s: &CoreState, who: Seat, system: Option<System>) -> i64 {
        self.0.hooks.direct.firmament_plots_assail_space(s, self.0, who, system)
    }
    fn unit_dice(&self, s: &CoreState, unit: CombatUnit, dice: i64) -> i64 {
        self.0.hooks.general.unit_dice(s, self.0, unit, dice)
    }
    fn borrowed_stats(&self, s: &CoreState, who: Seat, system: System) -> Option<Borrowed> {
        let b = self.0.hooks.combat.borrowed_stats(s, self.0, who, system)?;
        // The hook names the ship by type: the first ship of that type in board order borrows.
        let piece = s.board().sys(system).units.iter().find(|p| p.owner() == who && p.kind() == b.unit).copied()?;
        Some(Borrowed { unit: piece, hits_on: b.hits_on.map(i64::from), barrage: b.barrage.map(|(v, d)| (i64::from(v), i64::from(d))) })
    }
    fn may_sustain_modules(&self, _: &CoreState, _: CombatUnit) -> bool {
        true
    }
    fn bastion_extra_die(&self, s: &CoreState, piece: Piece) -> i64 {
        self.0.hooks.direct.bastion_extra_die(s, piece) as i64
    }
    fn round_agent_extra_die(&self, s: &CoreState, system: System, who: Seat, unit: Unit, index: usize) -> i64 {
        self.0.hooks.direct.borrowed_round_agents_extra_die_for(
            s,
            self.0,
            system,
            None,
            who,
            unit,
            index,
            s.meta().combat_round_seq,
        )
    }
}

/// What a combat rule reads: the state, the kernel (content scope, hook table) and the faction seam.
#[derive(Clone, Copy)]
pub struct Rd<'a> {
    pub s: &'a CoreState,
    pub k: &'a Kernel,
    pub f: &'a dyn Faction,
    /// Law's Order: every law is blank this turn (`laws::blanked`; the caller computes it).
    pub blanked: bool,
}

impl<'a> Rd<'a> {
    #[must_use]
    pub fn new(s: &'a CoreState, k: &'a Kernel, f: &'a dyn Faction) -> Self {
        Self { s, k, f, blanked: false }
    }

    /// The same read with laws blank or not.
    #[must_use]
    pub fn with_blanked(mut self, blanked: bool) -> Self {
        self.blanked = blanked;
        self
    }

    /// The same read over another state (a played-out copy).
    #[must_use]
    pub fn over(self, s: &'a CoreState) -> Self {
        Self { s, ..self }
    }

    /// The source scope as the tables read it.
    #[must_use]
    pub fn scope(&self) -> Scope {
        Scope(u32::from(self.k.sources.0))
    }

    /// The unit's table row when the unit is in the game's scope (the string `catalogue(content,
    /// sources).get(id)`), else `None`.
    #[must_use]
    pub fn data(&self, unit: Unit) -> Option<&'static crate::generated::UnitData> {
        self.scope().unit(unit)
    }

    /// `UnitType::is_ship` (false for a unit out of scope).
    #[must_use]
    pub fn is_ship(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.ship)
    }

    /// `UnitType::is_fighter`.
    #[must_use]
    pub fn is_fighter(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.base_type == UnitBaseType::Fighter)
    }
}

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
    let chosen = rd.f.alastor_participants(rd.s, player, system);
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

/// The roll bonus this seat's fighters carry in the current combat round (Fighter Prototype and the
/// Ixth law).
#[must_use]
pub fn fighter_bonus_now(rd: Rd<'_>, player: Seat) -> i64 {
    let round = rd.s.meta().combat_round_seq;
    let prototypes = rd.s.players().by_seat(player).map_or(0, |p| {
        2 * p.round_marks.of(RoundKind::FighterBonus).filter(|r| *r == round).count() as i64
    });
    prototypes + rd.f.law_fighter_bonus(rd.s, player)
}

/// `combat::effective_hits_on`: the threshold a unit needs this round after its round-scoped effects.
#[must_use]
pub fn effective_hits_on(rd: Rd<'_>, player: Seat, unit: Piece) -> Option<i64> {
    effective_from(rd, player, unit, hits_on(rd, unit.kind())?)
}

/// [`effective_hits_on`] for a unit rolling on `threshold` instead of its own printed value.
#[must_use]
pub fn effective_from(rd: Rd<'_>, player: Seat, unit: Piece, threshold: i64) -> Option<i64> {
    let s = rd.s;
    let meta = s.meta();
    let morale = s.players().by_seat(player).is_some_and(|p| p.combat_timers.combat_bonus_round.is(meta.combat_round_seq));
    let faction = rd.f.combat_modifier(s, player, CombatCtx::Space);
    let fighter_bonus = if rd.is_fighter(unit.kind()) { fighter_bonus_now(rd, player) } else { 0 };
    // 59.5: the defender in a nebula applies +1 to each combat roll of their ships.
    let nebula_defender = meta
        .active_system
        .filter(|_| meta.active != Some(player))
        .and_then(|system| rd.scope().system(system))
        .is_some_and(|d| d.anomalies.nebula);
    let module = rd.f.unit_roll_modifier(
        s,
        CombatUnit { owner: player, system: meta.active_system, planet: None, unit: unit.kind(), ctx: CombatCtx::Space },
    );
    let mahact = rd.f.mahact_flagship_bonus(s, player, unit.kind());
    let nekro = rd.f.nekro_mech_bonus(s, player, unit.kind(), meta.active_system);
    let assail = rd.f.assail_space(s, player, meta.active_system);
    Some(
        threshold
            - i64::from(morale)
            - faction
            - fighter_bonus
            - i64::from(nebula_defender)
            - module
            - mahact
            - nekro
            - assail,
    )
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

/// Whether `player` has 0.0.1 among its ships in `system`.
#[must_use]
pub fn forces_non_fighters(rd: Rd<'_>, player: Seat, system: System) -> bool {
    ships_of(rd, player, system).iter().any(|p| rd.f.flagship_has_text(rd.s, player, p.kind(), FlagshipText::L1z1x))
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
    let borrower = rd.f.borrowed_stats(s, player, system).and_then(|b| b.hits_on.map(|v| (b.unit, v)));
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
        let mut dice = rd.f.unit_dice(
            s,
            CombatUnit { owner: player, system: Some(system), planet: None, unit: unit.kind(), ctx: CombatCtx::Space },
            base_dice,
        );
        if !extra_die_added && extra_die.is_some_and(|selected| selected == unit.kind()) {
            dice += 1;
            extra_die_added = true;
        }
        dice += rd.f.bastion_extra_die(s, unit);
        dice += rd.f.round_agent_extra_die(s, system, player, unit.kind(), unit_index);
        let jolnar = rd.f.flagship_has_text(s, player, unit.kind(), FlagshipText::Jolnar);
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

/// `relics::grants_sustain`: Metali Void Shielding.
#[must_use]
pub fn relic_grants_sustain(s: &CoreState, seat: Seat) -> bool {
    s.players().by_seat(seat).is_some_and(|p| p.cards.relics.contains(&Relic::MetaliVoidShielding))
}

/// A ship of its own accord: a ship with SUSTAIN DAMAGE, or any non-fighter ship under Metali Void
/// Shielding.
fn sustains_by_itself(rd: Rd<'_>, seat: Seat, unit: Unit) -> bool {
    rd.is_ship(unit)
        && (rd.data(unit).is_some_and(|d| d.sustain_damage) || (relic_grants_sustain(rd.s, seat) && !rd.is_fighter(unit)))
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
        && rd.k.hooks.combat.may_sustain(rd.s, rd.k, cu(CombatCtx::Space))
        && rd.f.may_sustain_modules(rd.s, cu(CombatCtx::Space))
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
        && !rd.k.hooks.combat.direct_hit_immune(
            rd.s,
            rd.k,
            CombatUnit { owner: player, system: Some(system), planet: None, unit, ctx: CombatCtx::Space },
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
    for r in rolls {
        let mask = r.rerolled.iter().filter(|i| **i < 64).fold(0_u64, |m, i| m | (1 << i));
        ledger.dice.push(RollRec {
            purpose: if r.rerolled.is_empty() { RollPurpose::SpaceCombat } else { RollPurpose::Reroll },
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
