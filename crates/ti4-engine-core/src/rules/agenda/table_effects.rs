//! The agenda effects that write the table and ask nothing (package B9; string `agenda_effects::effect_*`).
//!
//! Each [`Table`] is one effect (or one outcome of one effect) and [`Table::apply`] is its whole body:
//! * command tokens: Armed Forces Standardization (top up to 3/3/2), Clandestine Operations (spend), Fleet
//!   Regulations Against (a fleet token each, capped by the reinforcements), Shared Research Against (a token in
//!   each home system), Wormhole Reconstruction Against (a token in each wormhole system with a ship).
//! * trade goods: Economic Equality (all to zero, For pays five each), Articles of War Against (three to each
//!   For voter). Every gain is staged for the coordinator's flush, as the string `note_trade_goods_gained`.
//! * hands: Public Execution (the elected seat's hand and the speaker token), Conventions of War Against and
//!   Unconventional Measures Against (the voters' hands), Publicize Weapon Schematics Against (the War Sun owners).
//! * units: Arms Reduction For (down to two dreadnoughts and four cruisers each), Rearmament Agreement Against
//!   (every mech on a planet becomes an infantry), Colonial Redistribution's wipe aside, the four planet
//!   effects (Compensated Disarmament, Swords to Plowshares For, Core Mining, Demilitarized Zone). A ground force
//!   that falls is staged as a `private:#staged:ground:` row (cause `agenda:<alias>`), as
//!   `hooks_ground::stage_ground_force_destroyed`.
//! * Regulated Conscription does nothing when it passes.
//!
//! Not here (the adapter runs the string effect alone, one test per bail): the dice (Executive Sanctions, Ixthian
//! Artifact), the unit placements through the supply and the faction hooks (Rearmament For, Plowshares Against,
//! Colonial Redistribution), the draw of action cards (Unconventional Measures For), the relic deck (Minister of
//! Antiquities), the technology choice (Research Grant Reallocation), Galactic Crisis Pact, and the effects that
//! read the string map (Enforced Travel Ban, Wormhole Research).

use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::keys::{Atom, DestroyReason, MarkKey, MarkVal, Word};
use crate::query::{board, economy};
use crate::state::{Code, CoreState, EventKind, Key, Payload, Piece, PlanetFlags, SysFlags, Val};
use crate::tables::Scope;

/// What the effect reads besides the state: the voters of the ballot, the Mahact fleet pools (a faction mark
/// the typed reads take as an argument), the source scopes and two interned texts.
#[derive(Debug, Clone)]
pub struct EffectFacts {
    /// `ballot.voted_for(FOR)`, in the ballot's order.
    pub for_voters: SmallList<Seat, 8>,
    /// `ballot.voted_for(AGAINST)`.
    pub against_voters: SmallList<Seat, 8>,
    /// `tokens_held_by_mahact` of each seat.
    pub mahact_held: [i32; 8],
    /// The game's sources (the catalogue every unit read is made under).
    pub scope: Scope,
    /// The default sources (the one catalogue Rearmament reads under).
    pub default_scope: Scope,
    /// `"agenda"`, the source of a trade-good gain.
    pub source: Code,
    /// `"reinforcements"`, the pool of a placed command token.
    pub pool: Code,
}

/// Technologies whose printed name mentions a war sun (`owns_a_war_sun_technology`).
pub const WAR_SUN_TECHNOLOGIES: [Technology; 2] = [Technology::WarSun, Technology::PrototypeWarSunII];

/// One effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    /// Regulated Conscription: nothing.
    Nothing,
    /// Armed Forces Standardization: the elected seat is topped up to 3 tactic, 3 fleet, 2 strategic tokens.
    Standardize(Seat),
    /// Clandestine Operations: two tokens off every sheet (from the fullest pool), or one fleet token.
    Clandestine { both: bool },
    /// Fleet Regulations, Against: a fleet token for everyone.
    Regulations,
    /// Economic Equality: every seat's goods go to zero, then (`pay`) five each.
    Equality { pay: bool },
    /// Articles of War, Against: three goods for each For voter.
    ArticlesWar,
    /// Public Execution: the hand of the elected seat, and the speaker token when they hold it.
    Execution(Seat),
    /// Conventions of War, Against: the hands of the Against voters.
    ConventionsHands,
    /// Unconventional Measures, Against: the hands of the For voters.
    VotersHands,
    /// Publicize Weapon Schematics, Against: the hand of every War Sun owner.
    Schematics,
    /// Shared Research, Against: a command token in every home system.
    SharedResearch,
    /// Wormhole Reconstruction, Against: a command token in each wormhole system holding one of your ships.
    WormholeRecon,
    /// Arms Reduction, For.
    ArmsReduction,
    /// Rearmament Agreement, Against.
    Rearm,
    /// Compensated Disarmament.
    Disarmament(Planet),
    /// Swords to Plowshares, For.
    Plowshares,
    /// Core Mining.
    CoreMining(Planet),
    /// Demilitarized Zone.
    Demilitarized(Planet),
}

impl Table {
    /// The effect of `agenda` electing `outcome` (`Some(Outcome::For/Against)` as `for_`), and whether it ends
    /// deferred (`Effect::Deferred`); `None` for what this module does not express.
    #[must_use]
    pub fn of(agenda: Agenda, outcome: crate::keys::Outcome) -> Option<(Self, bool)> {
        use crate::keys::Outcome as O;
        use Agenda as A;
        let (table, deferred) = match (agenda, outcome) {
            (A::RegulatedConscription, _) => (Self::Nothing, false),
            (A::ArmedForcesStandardization, O::Seat(who)) => (Self::Standardize(who), false),
            (A::ClandestineOperations, O::For) => (Self::Clandestine { both: true }, false),
            (A::ClandestineOperations, _) => (Self::Clandestine { both: false }, false),
            (A::FleetRegulations, O::For) => (Self::Nothing, false),
            (A::FleetRegulations, O::Against) => (Self::Regulations, false),
            (A::EconomicEquality, O::For) => (Self::Equality { pay: true }, false),
            (A::EconomicEquality, O::Against) => (Self::Equality { pay: false }, false),
            (A::ArticlesOfWar, O::Against) => (Self::ArticlesWar, false),
            (A::ArticlesOfWar, O::For) => (Self::Nothing, false),
            (A::PublicExecution, O::Seat(who)) => (Self::Execution(who), true),
            (A::ConventionsOfWar, O::For) => (Self::Nothing, false),
            (A::ConventionsOfWar, O::Against) => (Self::ConventionsHands, false),
            (A::UnconventionalMeasures, O::Against) => (Self::VotersHands, false),
            (A::PublicizeWeaponSchematics, O::For) => (Self::Nothing, false),
            (A::PublicizeWeaponSchematics, O::Against) => (Self::Schematics, false),
            (A::SharedResearch, O::For) => (Self::Nothing, false),
            (A::SharedResearch, O::Against) => (Self::SharedResearch, false),
            (A::WormholeReconstruction, O::For) => (Self::Nothing, false),
            (A::WormholeReconstruction, O::Against) => (Self::WormholeRecon, false),
            (A::ArmsReduction, O::For) => (Self::ArmsReduction, false),
            (A::ArmsReduction, O::Against) => (Self::Nothing, true),
            (A::RearmamentAgreement, O::Against) => (Self::Rearm, false),
            (A::CompensatedDisarmament, O::Planet(p)) => (Self::Disarmament(p), false),
            (A::SwordsToPlowshares, O::For) => (Self::Plowshares, false),
            (A::CoreMining, O::Planet(p)) => (Self::CoreMining(p), false),
            (A::DemilitarizedZone, O::Planet(p)) => (Self::Demilitarized(p), false),
            _ => return None,
        };
        Some((table, deferred))
    }

    /// Whether the effect stages a ground row or a command token (the string only stages them while a faction
    /// module is seated; the adapter writes the staged events through the string's own gate).
    #[must_use]
    pub const fn reads_map_units(&self) -> bool {
        matches!(
            self,
            Self::ArmsReduction | Self::Rearm | Self::Disarmament(_) | Self::Plowshares | Self::CoreMining(_) | Self::Demilitarized(_)
        )
    }

    /// Run the effect. `false` when the position holds something the effect does not express (a galvanized
    /// ground force falls: its staged row has one more field); the state is then to be discarded.
    pub fn apply(&self, core: &mut CoreState, f: &EffectFacts) -> bool {
        match *self {
            Self::Nothing => {}
            Self::Standardize(who) => standardize(core, who),
            Self::Clandestine { both } => clandestine(core, both),
            Self::Regulations => {
                for seat in everyone(core).as_slice() {
                    let held = f.mahact_held[usize::from(seat.0) & 7];
                    if economy::tokens_in_reinforcements(core, *seat, held) > 0
                        && let Some(p) = core.players_mut().by_seat_mut(*seat)
                    {
                        p.econ.fleet_tokens += 1;
                    }
                }
            }
            Self::Equality { pay } => {
                for seat in everyone(core).as_slice() {
                    if let Some(p) = core.players_mut().by_seat_mut(*seat) {
                        p.econ.trade_goods = 0;
                    }
                }
                if pay {
                    for seat in everyone(core).as_slice() {
                        fx::gain_trade_goods_staged(core, *seat, 5, f.source);
                    }
                }
            }
            Self::ArticlesWar => {
                for seat in f.for_voters.as_slice() {
                    fx::gain_trade_goods_staged(core, *seat, 3, f.source);
                }
            }
            Self::Execution(who) => {
                clear_hand(core, who);
                if core.meta().speaker == who {
                    let order = clockwise_from(core, who);
                    if let Some(next) = order.as_slice().get(1) {
                        core.meta_mut().speaker = *next;
                    }
                }
            }
            Self::ConventionsHands => {
                for seat in f.against_voters.as_slice() {
                    clear_hand(core, *seat);
                }
            }
            Self::VotersHands => {
                for seat in f.for_voters.as_slice() {
                    clear_hand(core, *seat);
                }
            }
            Self::Schematics => {
                for seat in everyone(core).as_slice() {
                    let owns = core.players().by_seat(*seat).is_some_and(|p| WAR_SUN_TECHNOLOGIES.iter().any(|t| p.tech.owned.contains(*t)));
                    if owns {
                        clear_hand(core, *seat);
                    }
                }
            }
            Self::SharedResearch => {
                for seat in everyone(core).as_slice() {
                    let home = core.players().by_seat(*seat).and_then(|p| p.home_system.or(p.faction.data().home_system));
                    if let Some(home) = home {
                        place_token(core, *seat, home, f);
                    }
                }
            }
            Self::WormholeRecon => {
                for system in System::ALL {
                    let cell = core.board().sys(system);
                    if !cell.flags.has(SysFlags::TOUCHED) || !in_scope(f.scope, system.data().source) || system.data().wormholes.is_empty() {
                        continue;
                    }
                    for seat in everyone(core).as_slice() {
                        if board::has_space_units_of(core, system, *seat) {
                            place_token(core, *seat, system, f);
                        }
                    }
                }
            }
            Self::ArmsReduction => {
                for seat in everyone(core).as_slice() {
                    cull_ships(core, f.scope, *seat, UnitBaseType::Dreadnought, 2);
                    cull_ships(core, f.scope, *seat, UnitBaseType::Cruiser, 4);
                }
            }
            Self::Rearm => rearm(core, f.default_scope),
            Self::Disarmament(planet) => {
                let controller = board::control(core, planet);
                let destroyed = clear_planet(core, f, planet, |b| matches!(b, UnitBaseType::Infantry | UnitBaseType::Mech), None, Agenda::CompensatedDisarmament);
                let Ok(destroyed) = destroyed else { return false };
                if let Some(seat) = controller
                    && destroyed > 0
                    && core.players().by_seat(seat).is_some()
                {
                    fx::gain_trade_goods_staged(core, seat, i32::try_from(destroyed).unwrap_or(i32::MAX), f.source);
                }
            }
            Self::CoreMining(planet) => {
                if clear_planet(core, f, planet, |b| b == UnitBaseType::Infantry, Some(1), Agenda::CoreMining).is_err() {
                    return false;
                }
            }
            Self::Demilitarized(planet) => {
                if clear_planet(core, f, planet, |_| true, None, Agenda::DemilitarizedZone).is_err() {
                    return false;
                }
            }
            Self::Plowshares => return plowshares(core, f),
        }
        true
    }
}

fn everyone(core: &CoreState) -> SmallList<Seat, 8> {
    SmallList::from_slice(core.seats().seating_order())
}

/// `GameState::clockwise_from`: the seating order starting at `who` (the whole order when `who` is not seated).
fn clockwise_from(core: &CoreState, who: Seat) -> SmallList<Seat, 8> {
    let order = core.seats().seating_order();
    let Some(start) = order.iter().position(|s| *s == who) else { return SmallList::from_slice(order) };
    order[start..].iter().chain(&order[..start]).copied().collect()
}

fn in_scope(scope: Scope, source: crate::generated::Source) -> bool {
    scope.0 & crate::tables::source_bit(source) != 0
}

/// `agenda_effects::discard_hand`: the cards leave and are gone (the Ral Nel breakthrough takes them only in the
/// action phase, which an agenda never is: the adapter declines that phase).
fn clear_hand(core: &mut CoreState, who: Seat) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.hand.clear();
    }
}

fn standardize(core: &mut CoreState, who: Seat) {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return };
    for (tokens, target) in [(&mut p.econ.tactic_tokens, 3), (&mut p.econ.fleet_tokens, 3), (&mut p.econ.strategic_tokens, 2)] {
        if *tokens < target {
            *tokens = target;
        }
    }
}

/// The pools in `Player.POOLS` order.
fn pool(p: &mut crate::state::Player, index: usize) -> &mut i16 {
    match index {
        0 => &mut p.econ.tactic_tokens,
        1 => &mut p.econ.fleet_tokens,
        _ => &mut p.econ.strategic_tokens,
    }
}

fn clandestine(core: &mut CoreState, both: bool) {
    for seat in everyone(core).as_slice() {
        let Some(p) = core.players_mut().by_seat_mut(*seat) else { continue };
        if both {
            for _ in 0..2 {
                // `max_by_key`: the last of the fullest pools.
                let mut fullest = 0;
                for at in 1..3 {
                    if *pool(p, at) >= *pool(p, fullest) {
                        fullest = at;
                    }
                }
                let held = pool(p, fullest);
                if *held > 0 {
                    *held -= 1;
                }
            }
        } else if p.econ.fleet_tokens > 0 {
            p.econ.fleet_tokens -= 1;
        }
    }
}

/// A command token in `system`, staged as placed from the reinforcements when it was new.
fn place_token(core: &mut CoreState, who: Seat, system: System, f: &EffectFacts) {
    if fx::place_command_token(core, who, system) {
        let payload =
            Payload::new().with(Key::Player, Val::Seat(who)).with(Key::System, Val::System(system)).with(Key::Pool, Val::Code(f.pool));
        fx::stage(core, fx::event(EventKind::CommandTokenPlaced, payload));
    }
}

/// `agenda_effects::cull_ships`: the owner keeps `keep` ships of the base type; the others go in board order.
fn cull_ships(core: &mut CoreState, scope: Scope, who: Seat, base: UnitBaseType, keep: usize) {
    let matches = |p: &Piece| {
        let data = p.kind().data();
        p.owner() == who && data.base_type == base && in_scope(scope, data.source)
    };
    let held: usize = core.board().systems().map(|(_, cell)| cell.units.as_slice().iter().filter(|p| matches(p)).count()).sum();
    let mut over = held.saturating_sub(keep);
    if over == 0 {
        return;
    }
    for system in System::ALL {
        if over == 0 {
            break;
        }
        if !core.board().sys(system).units.as_slice().iter().any(&matches) {
            continue;
        }
        core.board_mut().sys_mut(system).units.retain(|p| {
            if over > 0 && matches(p) {
                over -= 1;
                return false;
            }
            true
        });
    }
}

/// Rearmament Agreement, Against: every mech on a planet is an undamaged infantry (a galvanized one stays so).
fn rearm(core: &mut CoreState, scope: Scope) {
    for planet in Planet::ALL {
        let is_mech = |p: &Piece| {
            let data = p.kind().data();
            data.base_type == UnitBaseType::Mech && in_scope(scope, data.source)
        };
        if !core.board().planet(planet).units.as_slice().iter().any(is_mech) {
            continue;
        }
        for piece in core.board_mut().planet_mut(planet).units.as_mut_slice() {
            if is_mech(piece) {
                *piece = Piece::new(Unit::InfantryI, piece.owner()).with_galvanized(piece.galvanized());
            }
        }
    }
}

/// The next `private:#staged:ground:` number: after the highest still staged.
fn next_ground(core: &CoreState) -> u32 {
    core.marks()
        .tail
        .iter()
        .filter_map(|(k, _)| if let MarkKey::PrivateStagedGround(n) = k { Some(*n) } else { None })
        .max()
        .map_or(0, |n| n + 1)
}

/// `hooks_ground::stage_ground_force_destroyed` for a piece the caller has removed. `Err` for a galvanized one.
fn stage_destroyed(core: &mut CoreState, system: System, planet: Planet, piece: Piece, cause: Agenda) -> Result<(), ()> {
    if piece.galvanized() {
        return Err(());
    }
    let key = MarkKey::PrivateStagedGround(next_ground(core));
    let row = MarkVal::Rec(vec![
        MarkVal::Atom(Atom::Word(Word::Destroyed)),
        MarkVal::Atom(Atom::System(system)),
        MarkVal::Atom(Atom::Planet(planet)),
        MarkVal::Atom(Atom::Seat(piece.owner())),
        MarkVal::Atom(Atom::Unit(piece.kind())),
        MarkVal::Atom(Atom::Word(if piece.sustained() { Word::True } else { Word::False })),
        MarkVal::Atom(Atom::Destroy(DestroyReason::Agenda(cause))),
    ]);
    core.marks_mut().tail.edit().insert(key, row);
    Ok(())
}

/// `agenda_effects::clear_planet`: remove the units whose base type `doomed` accepts (at most `limit`), stage
/// every ground force that fell, and return how many were destroyed.
fn clear_planet(
    core: &mut CoreState,
    f: &EffectFacts,
    planet: Planet,
    doomed: impl Fn(UnitBaseType) -> bool,
    limit: Option<usize>,
    cause: Agenda,
) -> Result<usize, ()> {
    let Some(system) = board::system_of(core, f.scope, planet) else { return Ok(0) };
    // `state.system_mut` creates the system's entry.
    core.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
    if !core.board().planet(planet).flags.has(PlanetFlags::UNITS_ENTRY) {
        return Ok(0);
    }
    let mut destroyed = 0;
    let mut fallen: SmallList<Piece, 40> = SmallList::new();
    core.board_mut().planet_mut(planet).units.retain(|p| {
        if limit.is_some_and(|cap| destroyed >= cap) {
            return true;
        }
        let data = p.kind().data();
        let hit = in_scope(f.scope, data.source) && doomed(data.base_type);
        if hit {
            destroyed += 1;
            if matches!(data.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) {
                fallen.push(*p);
            }
        }
        !hit
    });
    for piece in fallen.as_slice() {
        stage_destroyed(core, system, planet, *piece, cause)?;
    }
    Ok(destroyed)
}

/// Swords to Plowshares, For: half of every seat's infantry (rounded up) is bought back for goods.
fn plowshares(core: &mut CoreState, f: &EffectFacts) -> bool {
    for seat in everyone(core).as_slice() {
        let mut destroyed = 0usize;
        for (system, planet) in board::controlled_planets(core, *seat).as_slice() {
            // `state.system_mut` creates the entry of every system a held planet sits in.
            core.board_mut().sys_mut(*system).flags.set(SysFlags::TOUCHED, true);
            let held: SmallList<usize, 40> = core
                .board()
                .planet(*planet)
                .units
                .as_slice()
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    let data = p.kind().data();
                    p.owner() == *seat && data.base_type == UnitBaseType::Infantry && in_scope(f.scope, data.source)
                })
                .map(|(i, _)| i)
                .collect();
            let losses = held.len().div_ceil(2);
            if !core.board().planet(*planet).flags.has(PlanetFlags::UNITS_ENTRY) {
                continue;
            }
            // The lowest `losses` positions, removed from the highest down; staged in that order.
            let mut fallen: SmallList<Piece, 40> = SmallList::new();
            for at in held.as_slice().iter().take(losses).rev() {
                fallen.push(core.board_mut().planet_mut(*planet).units.remove(*at));
            }
            for piece in fallen.as_slice() {
                if stage_destroyed(core, *system, *planet, *piece, Agenda::SwordsToPlowshares).is_err() {
                    return false;
                }
            }
            destroyed += losses;
        }
        if core.players().by_seat(*seat).is_some() {
            // The goods are added, then announced with the same amount; a gain of zero announces nothing.
            fx::gain_trade_goods_staged(core, *seat, i32::try_from(destroyed).unwrap_or(i32::MAX), f.source);
        }
    }
    true
}
