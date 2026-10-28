//! Invasion (LRR 49) and the ground combat under LRR 42: the typed port of `ti4-engine/src/invasion.rs`
//! (package B2).
//!
//! Layout, in the order of the string module:
//!
//! * [`Rd`]: the read bundle (state, kernel, laws blank or not), shared with combat from `kernel::read`
//!   (an L4 area does not import another, `check_layers.py`);
//! * bombardment (49.1): [`bombardable`], [`bombarders`], [`bombard_plan`], the rolls of each planet before
//!   any die is drawn;
//! * landing and committing ground forces, and the ground combat itself follow in the later sections.
//!
//! Nothing here asks a question or emits an event: those go through the host the later sections take.

pub mod absorb;
pub mod bombard;
pub mod commit;
pub mod defense;
pub mod hits;
pub mod roll;
pub mod round;
pub mod stages;

use crate::containers::SeatSet;
use crate::generated::{Leader, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::keys::MarkKey;
use crate::kernel::seam::{data as seam_data, Clamp, RollKind, RowKind, SilenceCtx, SilenceSite, Source};
use crate::kernel::seam::RollSite;
use crate::kernel::hooks::{CombatCtx, CombatUnit};
use crate::query::board as qboard;
use crate::query::laws as qlaws;
use crate::state::{CoreState, Piece, RoundKind};
use crate::tables::{Flags, SystemFlags};

pub use crate::kernel::read::Rd;

impl Rd<'_> {
    fn lost(&self, owner: Seat, system: System) -> bool {
        self.k.hooks.seam.silenced.decide(
            self.q(),
            &SilenceSite { owner, system, unit: None, ctx: SilenceCtx::Space },
            false,
        )
    }
}

/// `UnitType::is_ground_force`: infantry and mechs by base type, and the Hel-Titan PDS.
#[must_use]
pub fn is_ground_force(rd: Rd<'_>, unit: Unit) -> bool {
    rd.data(unit).is_some_and(|d| matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech))
        || matches!(unit, Unit::HelTitanI | Unit::HelTitanII)
}

/// The planets whose unit lists live in `system`, in planet (text) order.
fn planets_sorted(s: &CoreState, system: System) -> Vec<Planet> {
    let mut v: Vec<Planet> = qboard::planets_of(s, system).collect();
    v.sort_unstable();
    v
}

/// Whether the system is an entropic scar (`entropic_scars::is_scar`).
fn is_scar(rd: Rd<'_>, system: System) -> bool {
    rd.scope().system(system).is_some_and(|d| d.has(SystemFlags::SCAR))
}

// -- bombardment (LRR 49.1) ------------------------------------------------------------------------

/// Whether `owner` holds the L1Z1X commander's ability by an unlocked commander or a granted mark. The other
/// ways to hold it (the Mahact Imperia, an "An" promissory note) belong to package B7's
/// `has_commander_ability`; the adapter leaves a position where one is in play to the string answer.
#[must_use]
pub fn ignores_planetary_shield(s: &CoreState, owner: Seat) -> bool {
    s.marks().tail.contains_key(&MarkKey::CommanderAbility(owner, Leader::L1z1xcommander))
        || s.players().by_seat(owner).and_then(|p| p.leaders.status.get(&Leader::L1z1xcommander).copied())
            == Some(crate::state::LeaderStatus::Unlocked)
}

/// Whether the unit is the Letnev flagship's text (Arc Secundus, or a Nekro Alastor lent it by its Z token).
fn has_arc_secundus_text(s: &CoreState, who: Seat, unit: Unit) -> bool {
    use crate::generated::{Breakthrough, Faction};
    use crate::keys::atom::Atom;
    use crate::keys::MarkVal;
    unit == Unit::ArcSecundus
        || (unit == Unit::TheAlastor
            && s.players().by_seat(who).is_some_and(|p| {
                p.faction == Faction::TheNekroVirus && p.cards.breakthrough == Some(Breakthrough::ValefarAssimilatorZ)
            })
            && matches!(
                s.marks().tail.get(&MarkKey::NekroToken(who)),
                Some(MarkVal::List(items))
                    if items.iter().any(|i| matches!(i, MarkVal::Atom(Atom::Faction(f)) if *f == Faction::TheBaronyOfLetnev))
            ))
}

/// `invasion::bombardable`: 15.1f, Planetary Shield makes a planet immune to bombardment, unless a war sun,
/// Arc Secundus, the L1Z1X commander or a Disable strips it; Conventions of War outranks all of them.
#[must_use]
pub fn bombardable(rd: Rd<'_>, system: System, planet: Planet, invader: Seat) -> bool {
    if qlaws::bombardment_forbidden(rd.s, rd.blanked, rd.scope(), planet) {
        return false;
    }
    let board = rd.s.board();
    let own = |p: &&Piece| p.owner() == invader;
    if board.sys(system).units.iter().filter(own).any(|p| rd.data(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Warsun)) {
        return true;
    }
    if board.sys(system).units.iter().filter(own).any(|p| has_arc_secundus_text(rd.s, invader, p.kind())) {
        return true;
    }
    if rd.l1z1x_commander.unwrap_or_else(|| ignores_planetary_shield(rd.s, invader)) {
        return true;
    }
    // Disable: only cards played for this invasion count; a shield survives only while every Disable in play
    // belongs to the shield's own owner.
    let activation = rd.s.meta().activation_seq;
    let disabled = |holder: Seat| rd.s.players().by_seat(holder).is_some_and(|p| p.round_marks.contains(RoundKind::DisableInvasion, activation));
    !board.planet(planet).units.iter().any(|u| {
        if !rd.data(u.kind()).is_some_and(|d| d.planetary_shield) {
            return false;
        }
        if rd.lost(u.owner(), system) {
            return false;
        }
        !rd.s.players().iter().any(|p| p.seat != u.owner() && disabled(p.seat))
    })
}

/// `invasion::bombarders`: the invader's units that may bombard `planet`: the units in the space area, and the
/// L1Z1X Anihilator on any other planet of the system.
#[must_use]
pub fn bombarders(s: &CoreState, system: System, planet: Planet, invader: Seat) -> Vec<Piece> {
    let mut found: Vec<Piece> = s.board().sys(system).units.iter().filter(|p| p.owner() == invader).copied().collect();
    for other in planets_sorted(s, system) {
        if other == planet {
            continue;
        }
        found.extend(s.board().planet(other).units.iter().filter(|u| u.owner() == invader && u.kind() == Unit::Anihilator).copied());
    }
    found
}

/// One bombarding unit's roll: the unit, the number the dice need and how many dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BombardRoll {
    pub unit: Unit,
    pub hits_on: i64,
    pub dice: usize,
}

/// What `invasion::roll_bombard_plan` rolls on one planet, before any die is drawn: the rolls in order, how many
/// ground forces of other players stand there and whose they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetBombard {
    pub planet: Planet,
    pub rolls: Vec<BombardRoll>,
    pub held: usize,
    pub victims: SeatSet,
}

/// The ten-sided die (`dice::SIDES`).
pub const DIE_SIDES: u32 = 10;

/// One bombarding unit's dice as drawn: the planet it fires at, the unit, the hit number and the faces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BombardDice {
    pub planet: Planet,
    pub unit: Unit,
    pub hits_on: u32,
    pub faces: Vec<u32>,
}

/// The dice of `roll_bombard_plan`, drawn from `rng`'s dice stream: every roll of the plan in order (planet by
/// planet, unit by unit), `dice` faces each, exactly as the string loop's `dice.roll_by` draws them.
pub fn roll_bombard(plan: &[PlanetBombard], rng: &mut crate::state::Rng, seeds: &dyn crate::state::SeedSource) -> Vec<BombardDice> {
    use crate::state::RngDomain;
    let mut out = Vec::new();
    for planet in plan {
        for roll in &planet.rolls {
            let faces = (0..roll.dice).map(|_| rng.die(RngDomain::Dice, seeds, DIE_SIDES)).collect();
            out.push(BombardDice { planet: planet.planet, unit: roll.unit, hits_on: u32::try_from(roll.hits_on).unwrap_or(u32::MAX), faces });
        }
    }
    out
}

/// Whether `seat` has Plasma Scoring (the data row of the technology).
fn has_plasma_scoring(rd: Rd<'_>, owner: Seat) -> bool {
    seam_data::COMBAT_ROWS
        .iter()
        .filter(|r| matches!(r.kind, RowKind::ExtraDice(_)) && r.source == Source::Technology(crate::generated::Technology::PlasmaScoring))
        .any(|r| r.times(rd.q(), owner, SeatSet::EMPTY, None, 0) > 0)
}

/// `invasion::roll_bombard_plan`'s rolls (49.1): for every planet that can be bombarded and holds ground forces
/// of other players, each bombarding unit's value and dice. Entropic scar rules 2 and 4 and lost abilities end
/// the bombardment; Blitz grants a die to non-fighter ships; Plasma Scoring adds a die to the first roller; a
/// galvanized unit rolls one more; Bunker lowers the result against the controller's planet; Assail raises it.
#[must_use]
pub fn bombard_plan(rd: Rd<'_>, system: System, invader: Seat) -> Vec<PlanetBombard> {
    if is_scar(rd, system) || rd.lost(invader, system) {
        return Vec::new();
    }
    let activation = rd.s.meta().activation_seq;
    let blitzed = rd.s.players().by_seat(invader).is_some_and(|p| p.round_marks.contains(RoundKind::BlitzInvasion, activation));
    let mut plasma = has_plasma_scoring(rd, invader);
    let mut plan = Vec::new();
    for planet in planets_sorted(rd.s, system) {
        if !bombardable(rd, system, planet, invader) {
            continue;
        }
        let defenders: Vec<Piece> = rd
            .s
            .board()
            .planet(planet)
            .units
            .iter()
            .filter(|u| u.owner() != invader && is_ground_force(rd, u.kind()))
            .copied()
            .collect();
        if defenders.is_empty() {
            continue;
        }
        let mut foes = SeatSet::EMPTY;
        let mut victims = SeatSet::EMPTY;
        for d in &defenders {
            foes.insert(d.owner());
            victims.insert(d.owner());
        }
        let mut rolls = Vec::new();
        for unit in bombarders(rd.s, system, planet, invader) {
            let Some(data) = rd.data(unit.kind()) else { continue };
            let (value, count) = match data.bombardment {
                Some(r) if r.hits_on > 0 => (i64::from(r.hits_on), usize::from(r.dice)),
                _ if blitzed && data.ship && data.base_type != UnitBaseType::Fighter => (6, 1),
                _ => continue,
            };
            if count == 0 {
                continue;
            }
            let count = count + usize::from(std::mem::take(&mut plasma)) + usize::from(unit.galvanized());
            let bunker = qboard::control(rd.s, planet)
                .and_then(|c| rd.s.players().by_seat(c))
                .map_or(0, |p| 4 * i64::try_from(p.round_marks.of(RoundKind::BunkerInvasion).filter(|n| *n == activation).count()).unwrap_or(i64::MAX));
            let site = RollSite {
                unit: CombatUnit { owner: invader, system: Some(system), planet: Some(planet), unit: unit.kind(), ctx: CombatCtx::Space },
                kind: RollKind::CannonAssail,
                round: rd.s.meta().combat_round_seq,
                unit_index: 0,
                foes,
            };
            let assail = rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value;
            rolls.push(BombardRoll { unit: unit.kind(), hits_on: (value + bunker - assail).max(1), dice: count });
        }
        plan.push(PlanetBombard { planet, rolls, held: defenders.len(), victims });
    }
    plan
}

// -- landing and committing ground forces (LRR 49.2) -----------------------------------------------

/// `invasion::landable`: the invader's ground forces in the space area of `system`, in board order.
#[must_use]
pub fn landable(rd: Rd<'_>, player: Seat, system: System) -> Vec<Piece> {
    rd.s.board()
        .sys(system)
        .units
        .iter()
        .filter(|p| p.owner() == player && rd.data(p.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) || matches!(p.kind(), Unit::HelTitanI | Unit::HelTitanII)))
        .copied()
        .collect()
}

/// `invasion::landable_planets`: the planets of `system` a ground force may land on now, in the order of the
/// system's record, then the planets placed there during play. Mecatol Rex stays off the table while the
/// custodians token sits there (27.1), a space station takes no ground forces, nor does the elected
/// Demilitarized Zone, nor Custodia Vigilia.
#[must_use]
pub fn landable_planets(rd: Rd<'_>, system: System) -> Vec<Planet> {
    let scope = rd.scope();
    let in_scope = |p: &Planet| scope.0 & crate::tables::source_bit(p.data().source) != 0;
    let custodians = rd.s.meta().flags.has(crate::state::MetaFlags::CUSTODIANS_REMOVED);
    let placed = rd.s.board().planets().filter(|(_, c)| c.placed_in == Some(system)).map(|(p, _)| p);
    system
        .data()
        .planets
        .iter()
        .copied()
        .chain(placed)
        .filter(|p| *p != Planet::CustodiaVigilia)
        .filter(in_scope)
        .filter(|p| *p != Planet::MecatolRex || custodians)
        .filter(|p| !p.data().traits.contains(&crate::generated::PlanetTrait::Spacestation))
        .filter(|p| !qlaws::planet_is_demilitarized(rd.s, rd.blanked, *p))
        .collect()
}

/// One ground force that may be committed: the unit, where it stands when that is not the space area of the
/// active system (a faction's commit candidate), and whether it is a ship committed as a ground force for the
/// moment (a temporary space commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Troop {
    pub piece: Piece,
    pub origin: Option<(System, Planet)>,
    pub temporary: bool,
}

impl Troop {
    /// A ground force standing in the space area.
    #[must_use]
    pub const fn space(piece: Piece) -> Self {
        Self { piece, origin: None, temporary: false }
    }
}

/// `invasion::commit_options`' landings: one option per distinguishable landing (unit type, damage, origin, planet)
/// over the caller's `troops` and `planets` (already without the planets whose toll the invader cannot pay), troops
/// first, then the end of the sequence. Moving a unit onto the planet it already stands on is no landing.
#[must_use]
pub fn commit_opts_from(system: System, troops: &[Troop], planets: &[Planet]) -> crate::choice::Opts {
    use crate::choice::invasion::Opt as IOpt;
    let mut opts = crate::choice::Opts::default();
    type Landing = (Unit, bool, Planet, Option<(System, Planet)>);
    let mut seen: Vec<Landing> = Vec::new();
    for (index, troop) in troops.iter().enumerate() {
        let unit = troop.piece;
        for planet in planets {
            if troop.origin == Some((system, *planet)) {
                continue;
            }
            let key = (unit.kind(), unit.sustained(), *planet, troop.origin);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            opts.push(crate::choice::Opt::Invasion(IOpt::Commit { index: index as u16, planet: *planet, unit: unit.kind(), damaged: unit.sustained(), from: troop.origin }));
        }
    }
    opts.push(crate::choice::Opt::Invasion(IOpt::DoneCommitting));
    opts
}

/// [`commit_opts_from`] for troops standing in the space area (no origin).
#[must_use]
pub fn commit_opts(troops: &[Piece], planets: &[Planet]) -> crate::choice::Opts {
    // The system only matters to origins, which a space troop has none of.
    let troops: Vec<Troop> = troops.iter().map(|p| Troop::space(*p)).collect();
    commit_opts_from(System::MecatolRex18, &troops, planets)
}

// -- hits on ground forces (LRR 15.2, 87) ------------------------------------------------------------

/// What one hit on a player's ground forces does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundHit {
    /// An undamaged unit with SUSTAIN DAMAGE absorbs it.
    Sustained(Piece),
    /// This unit is destroyed.
    Destroyed(Piece),
    /// Nothing was left to take it (15.2a).
    Wasted,
}

fn combat_unit(owner: Seat, system: System, planet: Planet, unit: Unit) -> CombatUnit {
    CombatUnit { owner, system: Some(system), planet: Some(planet), unit, ctx: CombatCtx::Ground }
}

/// `invasion::is_ground_force_here`: a ground force by its type, or made one for the moment by a faction.
#[must_use]
pub fn is_ground_force_here(rd: Rd<'_>, who: Seat, system: System, planet: Planet, unit: Piece) -> bool {
    is_ground_force(rd, unit.kind()) || rd.k.hooks.ground.temporary_ground_force(rd.s, rd.k, who, system, planet, unit)
}

/// `invasion::can_sustain_here`: the unit has SUSTAIN DAMAGE, is undamaged and no faction forbids it.
#[must_use]
pub fn can_sustain_here(rd: Rd<'_>, unit: Piece, system: System, planet: Planet) -> bool {
    !unit.sustained()
        && is_ground_force(rd, unit.kind())
        && rd.data(unit.kind()).is_some_and(|d| d.sustain_damage)
        && rd.k.hooks.ground.may_sustain(rd.s, rd.k, combat_unit(unit.owner(), system, planet, unit.kind()))
}

/// `invasion::ground_hit_eligible`: a hit of a unit ability skips the units immune to those.
#[must_use]
pub fn ground_hit_eligible(rd: Rd<'_>, system: System, planet: Planet, unit: Piece, unit_ability: bool) -> bool {
    !unit_ability || !rd.k.hooks.combat.ability_hit_immune(rd.s, rd.k, combat_unit(unit.owner(), system, planet, unit.kind()))
}

/// `invasion::apply_ground_hit_with_origin`'s decision: an undamaged unit with SUSTAIN DAMAGE absorbs the hit;
/// otherwise the cheapest ground force is destroyed, a damaged unit before a fresh one of the same cost, then
/// by type id. Structures never take it (KD-2). Shared by ground combat, bombardment, Harrow and space cannon
/// defense.
#[must_use]
pub fn ground_hit(rd: Rd<'_>, system: System, planet: Planet, player: Seat, unit_ability: bool) -> GroundHit {
    let forces: Vec<Piece> = rd
        .s
        .board()
        .planet(planet)
        .units
        .iter()
        .filter(|u| u.owner() == player)
        .filter(|u| is_ground_force_here(rd, player, system, planet, **u))
        .filter(|u| ground_hit_eligible(rd, system, planet, **u, unit_ability))
        .copied()
        .collect();
    if let Some(sturdy) = forces.iter().find(|u| can_sustain_here(rd, **u, system, planet)) {
        return GroundHit::Sustained(*sturdy);
    }
    let cost = |u: &Piece| rd.data(u.kind()).and_then(|d| d.cost_halves).unwrap_or(0);
    let doomed = forces.iter().min_by(|a, b| {
        cost(a)
            .cmp(&cost(b))
            .then(b.sustained().cmp(&a.sustained()))
            .then(a.kind().cmp(&b.kind()))
    });
    doomed.map_or(GroundHit::Wasted, |d| GroundHit::Destroyed(*d))
}

/// The unit list of `planet` after `hit` landed on it: the sustained unit is marked damaged, the destroyed one
/// leaves (the first equal piece of the list, as `replace_planet_unit` and `remove_from_planet` do).
pub fn apply_ground_hit(s: &mut CoreState, planet: Planet, hit: GroundHit) {
    let units = &mut s.board_mut().planet_mut(planet).units;
    match hit {
        GroundHit::Sustained(p) => {
            if let Some(at) = units.iter().position(|u| *u == p) {
                units.as_mut_slice()[at] = p.with_sustained(true);
            }
        }
        GroundHit::Destroyed(p) => {
            if let Some(at) = units.iter().position(|u| *u == p) {
                units.remove(at);
            }
        }
        GroundHit::Wasted => {}
    }
}

/// `invasion::take_bombard_hits`: `produced` hits on `owner`'s ground forces on `planet`, one after the other
/// (a sustain cancels the further hits Non-Euclidean Shielding adds). Returns the state after and how many
/// ground forces were destroyed. Bombardment is a unit ability, so units immune to those are skipped.
#[must_use]
pub fn take_hits(rd: Rd<'_>, system: System, planet: Planet, owner: Seat, produced: usize, unit_ability: bool) -> (CoreState, usize) {
    let mut s = rd.s.clone();
    let mut destroyed = 0;
    let mut remaining = produced;
    while remaining > 0 {
        remaining -= 1;
        let hit = ground_hit(Rd { s: &s, ..rd }, system, planet, owner, unit_ability);
        match hit {
            GroundHit::Destroyed(_) => destroyed += 1,
            GroundHit::Sustained(_) => {
                let extra = match seam_data::cancel_on_sustain(Rd { s: &s, ..rd }.q(), &rd.k.hooks.seam.rows, owner) {
                    n if n > 0 => n as usize - 1,
                    _ => 0,
                };
                remaining = remaining.saturating_sub(extra);
            }
            GroundHit::Wasted => {}
        }
        apply_ground_hit(&mut s, planet, hit);
    }
    (s, destroyed)
}

// -- ground combat rolls (LRR 42) --------------------------------------------------------------------

/// `RULES_FACTION_EFFECTS`.
const RULES_FACTION_EFFECTS: u32 = 16;

fn ground_site(owner: Seat, system: System, planet: Planet, unit: Unit, index: u16, round: u32) -> RollSite {
    RollSite {
        unit: combat_unit(owner, system, planet, unit),
        kind: RollKind::Ground,
        round,
        unit_index: index,
        foes: SeatSet::EMPTY,
    }
}

/// `invasion::ground_combat_value`: a unit's threshold in a ground combat, the printed number less the shift of
/// every provider (the faction modules, Shield Paling, Assail, the Nekro mech): `None` for a unit that does not
/// roll.
#[must_use]
pub fn ground_combat_value(rd: Rd<'_>, player: Seat, system: System, planet: Planet, unit: Unit) -> Option<i64> {
    let printed = rd.data(unit).and_then(|d| d.combat).map(|r| i64::from(r.hits_on)).filter(|v| *v > 0)?;
    let site = ground_site(player, system, planet, unit, 0, rd.s.meta().combat_round_seq);
    Some(printed - rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, crate::kernel::seam::Clamp::None).value)
}

/// One value's pool of a side's ground roll: how many dice and which unit types sit behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundPool {
    pub value: i64,
    pub dice: i64,
    pub types: Vec<(Unit, u32)>,
}

/// `invasion::roll_ground`'s pools, before any die is drawn: the ground forces of `player` on `planet` grouped by
/// threshold (ascending), with the dice of each (the unit's own, the galvanized die, the Sol agent's die once, the
/// round agents' extra die).
#[must_use]
pub fn ground_pools(rd: Rd<'_>, player: Seat, system: System, planet: Planet) -> Vec<GroundPool> {
    let round = rd.s.meta().combat_round_seq;
    let roster: Vec<Piece> = rd
        .s
        .board()
        .planet(planet)
        .units
        .iter()
        .filter(|u| u.owner() == player)
        .filter(|u| is_ground_force_here(rd, player, system, planet, **u))
        .copied()
        .collect();
    let sol = rd
        .s
        .players()
        .by_seat(player)
        .and_then(|p| if p.combat_timers.extra_die_round.is(round) { p.combat_timers.extra_die_unit } else { None });
    let mut sol_added = false;
    let mut pools: Vec<GroundPool> = Vec::new();
    for (index, unit) in roster.into_iter().enumerate() {
        let Some(data) = rd.data(unit.kind()) else { continue };
        let Some(value) = ground_combat_value(rd, player, system, planet, unit.kind()) else { continue };
        let site = ground_site(player, system, planet, unit.kind(), index as u16, round);
        let base = data.combat.map_or(0, |r| i64::from(r.dice));
        let mut dice = rd.k.hooks.seam.dice.eval(rd.q(), &site, base, crate::kernel::seam::Clamp::None).value;
        dice += i64::from(unit.galvanized());
        if !sol_added && rd.s.meta().rules_version >= RULES_FACTION_EFFECTS && sol == Some(unit.kind()) {
            dice += 1;
            sol_added = true;
        }
        let at = match pools.binary_search_by(|p| p.value.cmp(&value)) {
            Ok(at) => at,
            Err(at) => {
                pools.insert(at, GroundPool { value, dice: 0, types: Vec::new() });
                at
            }
        };
        let pool = &mut pools[at];
        pool.dice += dice;
        match pool.types.binary_search_by(|(u, _)| u.cmp(&unit.kind())) {
            Ok(i) => pool.types[i].1 += 1,
            Err(i) => pool.types.insert(i, (unit.kind(), 1)),
        }
    }
    pools
}

// -- the custodians token (LRR 27) ---------------------------------------------------------------------

/// 27.2: six influence, paid before ground forces are committed.
pub const CUSTODIANS_COST: i64 = 6;

/// `invasion::custodians_cost`: six influence, or nothing when a faction says so (Winnu `blood_ties`).
#[must_use]
pub fn custodians_cost(rd: Rd<'_>, who: Seat) -> i64 {
    if rd.k.hooks.ground.custodians_free(rd.s, rd.k, who) { 0 } else { CUSTODIANS_COST }
}

/// The part of `invasion::custodians_removable` that reads the board: the token is still on Mecatol Rex, this
/// is Mecatol Rex, and the player can commit ground forces to land there (27.2a). Whether the player can pay is
/// the payment rules' (package B3).
#[must_use]
pub fn custodians_gate(rd: Rd<'_>, who: Seat, system: System) -> bool {
    !rd.s.meta().flags.has(crate::state::MetaFlags::CUSTODIANS_REMOVED)
        && matches!(system, System::MecatolRex18 | System::MecatolRex112)
        && !landable(rd, who, system).is_empty()
}
