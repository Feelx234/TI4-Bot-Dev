//! Invasion (LRR 49) and the ground combat under LRR 42: the typed port of `ti4-engine/src/invasion.rs`
//! (package B2).
//!
//! Layout, in the order of the string module:
//!
//! * [`Rd`]: the read bundle (state, kernel, laws blank or not), a copy of `rules::combat::Rd` because an
//!   L4 area does not import another (`check_layers.py`);
//! * bombardment (49.1): [`bombardable`], [`bombarders`], [`bombard_plan`], the rolls of each planet before
//!   any die is drawn;
//! * landing and committing ground forces, and the ground combat itself follow in the later sections.
//!
//! Nothing here asks a question or emits an event: those go through the host the later sections take.

use crate::containers::SeatSet;
use crate::generated::{Leader, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::keys::MarkKey;
use crate::kernel::seam::{data as seam_data, Clamp, RollKind, RowKind, SilenceCtx, SilenceSite, Source, Q};
use crate::kernel::seam::RollSite;
use crate::kernel::hooks::{CombatCtx, CombatUnit};
use crate::kernel::Kernel;
use crate::query::board as qboard;
use crate::query::laws as qlaws;
use crate::state::{CoreState, Piece, RoundKind};
use crate::tables::{Flags, Scope, SystemFlags};

/// What an invasion rule reads: the state and the kernel (content scope, hook table, seams).
#[derive(Clone, Copy)]
pub struct Rd<'a> {
    pub s: &'a CoreState,
    pub k: &'a Kernel,
    /// Law's Order: every law is blank this turn (`laws::blanked`; the caller computes it).
    pub blanked: bool,
}

impl<'a> Rd<'a> {
    #[must_use]
    pub fn new(s: &'a CoreState, k: &'a Kernel) -> Self {
        Self { s, k, blanked: false }
    }

    /// The seam reader over the same state and kernel.
    #[must_use]
    pub fn q(&self) -> Q<'a> {
        Q::new(self.s, self.k)
    }

    /// The same read with laws blank or not.
    #[must_use]
    pub fn with_blanked(mut self, blanked: bool) -> Self {
        self.blanked = blanked;
        self
    }

    /// The source scope as the tables read it.
    #[must_use]
    pub fn scope(&self) -> Scope {
        Scope(u32::from(self.k.sources.0))
    }

    /// The unit's table row when the unit is in the game's scope.
    #[must_use]
    pub fn data(&self, unit: Unit) -> Option<&'static crate::generated::UnitData> {
        self.scope().unit(unit)
    }

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
    if ignores_planetary_shield(rd.s, invader) {
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

/// `invasion::commit_options`' landings: one option per distinguishable landing (unit type, damage, planet) over
/// the caller's `troops` and `planets` (already without the planets whose toll the invader cannot pay), troops
/// first, then the end of the sequence. Only for troops standing in space (no origin).
#[must_use]
pub fn commit_opts(troops: &[Piece], planets: &[Planet]) -> crate::choice::Opts {
    use crate::choice::invasion::Opt as IOpt;
    let mut opts = crate::choice::Opts::default();
    let mut seen: Vec<(Unit, bool, Planet)> = Vec::new();
    for (index, unit) in troops.iter().enumerate() {
        for planet in planets {
            let key = (unit.kind(), unit.sustained(), *planet);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            opts.push(crate::choice::Opt::Invasion(IOpt::Commit { index: index as u16, planet: *planet, unit: unit.kind(), damaged: unit.sustained() }));
        }
    }
    opts.push(crate::choice::Opt::Invasion(IOpt::DoneCommitting));
    opts
}
