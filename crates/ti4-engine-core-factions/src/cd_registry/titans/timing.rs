//! titans: Awaken and Ouranos, the abilities of the activation window (lane M6d). Both are `SYSTEM_ACTIVATED / after` abilities of the activating
//! Titans seat, registered for every seat as the string does (`factions::titans::timing_abilities`).
//!
//! * Awaken (`ability:*:awaken:SYSTEM_ACTIVATED:after`): "You may replace each of those [sleeper] tokens with 1 PDS from your reinforcements" (with
//!   Hecatoncheires, 1 mech and 1 infantry instead). Every question is asked before the first placement; Coalescence is the activation mark
//!   `TitansCoalescence` the ground hook reads (`query::forced_combat_planets`).
//! * Ouranos (`unit:*:titans_flagship:SYSTEM_ACTIVATED:after`): "you may replace 1 of those PDS with this unit" (the Nekro flagship too, through the
//!   Z token's text).

use ti4_engine_core::choice::activation::{Ask as MAsk, Awakening, Opt as MOpt, Subtype as MSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::{board, fleet, laws, objectives, reach};
use ti4_engine_core::state::{CoreState, EventKind, Key};

use super::offer::{is_titans, sleeper_rows};
use crate::cd_registry::yin::timing::placed_form_overridden;
use crate::common::{flagship_has_text, scope};

/// Awaken.
pub const AWAKEN: Slot = Slot(50_001);
/// Ouranos.
pub const OURANOS: Slot = Slot(50_002);

/// The unit the seat places for `base`, and whether the box holds `planned + 1` of it beyond what the earlier tokens claimed (`room`).
fn room(s: &CoreState, k: &Kernel, who: Seat, base: UnitBaseType, planned: i64) -> bool {
    let sc = scope(k);
    placed_form_overridden(s, k, sc, who, base).is_some_and(|unit| fx::box_has(s, k, sc, who, unit) && fleet::remaining_all(s, sc, who, unit) - planned >= 1)
}

/// `hecatoncheires_possible`: the seat's mech is the Hecatoncheires and the box holds a mech beyond what is planned, and an infantry.
fn hecatoncheires_possible(s: &CoreState, k: &Kernel, who: Seat, planned_mechs: i64) -> bool {
    placed_form_overridden(s, k, scope(k), who, UnitBaseType::Mech) == Some(Unit::Hecatoncheires)
        && room(s, k, who, UnitBaseType::Mech, planned_mechs)
        && room(s, k, who, UnitBaseType::Infantry, 0)
}

/// `awakenings`: what each sleeper token in `system` may become, with the plastic the earlier tokens of the activation claimed. Tokens that can
/// become nothing are left out.
fn awakenings(s: &CoreState, k: &Kernel, who: Seat, system: System, claimed: &[(Planet, Awakening)]) -> Vec<(Planet, Vec<Awakening>)> {
    let count = |wanted: Awakening| i64::try_from(claimed.iter().filter(|(_, kind)| *kind == wanted).count()).unwrap_or(0);
    let (planned_pds, planned_mechs) = (count(Awakening::Pds), count(Awakening::MechAndInfantry));
    let blanked = reach::blanked(s);
    sleeper_rows(s)
        .into_iter()
        .filter(|(_, owner, at)| *owner == who && *at == system)
        .map(|(planet, _, _)| planet)
        .filter(|planet| !laws::planet_is_demilitarized(s, blanked, *planet) && !objectives::is_space_station(*planet))
        .filter_map(|planet| {
            let mut kinds = Vec::new();
            if laws::structure_allowed(s, blanked, who, planet, UnitBaseType::Pds) && room(s, k, who, UnitBaseType::Pds, planned_pds) {
                kinds.push(Awakening::Pds);
            }
            if hecatoncheires_possible(s, k, who, planned_mechs) {
                kinds.push(Awakening::MechAndInfantry);
            }
            (!kinds.is_empty()).then_some((planet, kinds))
        })
        .collect()
}

fn activated_by(c: &CondCx<'_>) -> Option<System> {
    let p = &c.event.payload;
    (p.seat(Key::Player) == Some(c.def.owner)).then(|| p.system(Key::System)).flatten()
}

fn awaken_when(c: &CondCx<'_>) -> bool {
    is_titans(c.state, c.def.owner) && activated_by(c).is_some_and(|system| !awakenings(c.state, c.kernel, c.def.owner, system, &[]).is_empty())
}

fn ctx(origin: ti4_engine_core::state::Code, subtype: MSub) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Other(origin)),
        subtype: Subtype::Reaction(RSub::Act(subtype)),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

/// Put the Coalescence mark of this activation on `planet` when the awakened units stand beside another player's.
fn note_coalescence(s: &mut CoreState, who: Seat, system: System, planet: Planet) {
    if board::planet_units(s, planet).iter().any(|p| p.owner() != who) {
        let seq = s.meta().activation_seq;
        s.marks_mut().tail.edit().insert(MarkKey::TitansCoalescence(planet), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(system))]));
    }
}

fn awaken(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    if eff.event.payload.seat(Key::Player) != Some(me) || !is_titans(eff.read().0, me) {
        return Ok(());
    }
    // Every question before the first placement.
    let mut chosen: Vec<(Planet, Awakening)> = Vec::new();
    let first = {
        let (s, k) = eff.read();
        awakenings(s, k, me, system, &chosen)
    };
    let lone = first.len() == 1;
    for (planet, _) in first {
        let now = {
            let (s, k) = eff.read();
            awakenings(s, k, me, system, &chosen)
        };
        let Some((_, kinds)) = now.into_iter().find(|(p, _)| *p == planet) else { continue };
        let kind = if lone && kinds.len() == 1 {
            kinds[0]
        } else {
            let mut opts: Opts = kinds.iter().map(|kind| Opt::Reaction(ROpt::Act(MOpt::Awaken(*kind)))).collect();
            opts.push(Opt::Decline);
            let choice = Choice { actor: me, ask: Ask::Reaction(RAsk::Act(MAsk::Awaken { planet })), ctx: ctx(eff.codes().activation.awaken, MSub::Awaken), opts };
            let Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Awaken(kind)))) = eff.ask(&choice) else { continue };
            if !kinds.contains(&kind) {
                continue;
            }
            kind
        };
        chosen.push((planet, kind));
    }
    let sc = scope(eff.kernel());
    let (s, k) = eff.state_and_kernel();
    for (planet, kind) in chosen {
        s.marks_mut().tail.edit().remove(&MarkKey::TitansSleeper(planet));
        let bases: &[UnitBaseType] = match kind {
            Awakening::Pds => &[UnitBaseType::Pds],
            Awakening::MechAndInfantry => &[UnitBaseType::Mech, UnitBaseType::Infantry],
        };
        for base in bases {
            let form = placed_form_overridden(s, k, sc, me, *base);
            let _ = fx::place_counted(s, k, sc, me, system, Some(planet), form, 1);
        }
        note_coalescence(s, me, system, planet);
    }
    Ok(())
}

// -- Ouranos -------------------------------------------------------------------------------------------

/// `deploy_flagship`: the Titans flagship, or the Nekro one when the Z token has switched the Titans flagship's text on.
fn deploy_flagship(s: &CoreState, who: Seat) -> Option<Unit> {
    if is_titans(s, who) {
        Some(Unit::Ouranos)
    } else {
        flagship_has_text(s, who, Unit::TheAlastor, Unit::Ouranos).then_some(Unit::TheAlastor)
    }
}

/// `ouranos_planets`: the planets of `system` holding a PDS of the seat, while the flagship is in the reinforcements.
fn ouranos_planets(s: &CoreState, k: &Kernel, who: Seat, system: System) -> Vec<Planet> {
    let Some(flagship) = deploy_flagship(s, who) else { return Vec::new() };
    let sc = scope(k);
    if !fx::box_has(s, k, sc, who, flagship) {
        return Vec::new();
    }
    let mut planets: Vec<Planet> = board::planets_of(s, system).collect();
    planets.sort_unstable();
    planets.retain(|planet| board::planet_units(s, *planet).iter().any(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Pds));
    planets
}

fn ouranos_when(c: &CondCx<'_>) -> bool {
    deploy_flagship(c.state, c.def.owner).is_some() && activated_by(c).is_some_and(|system| !ouranos_planets(c.state, c.kernel, c.def.owner, system).is_empty())
}

fn ouranos(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let Some(flagship) = deploy_flagship(eff.read().0, me) else { return Ok(()) };
    let planets = {
        let (s, k) = eff.read();
        ouranos_planets(s, k, me, system)
    };
    let planet = match planets.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let opts: Opts = many.iter().map(|p| Opt::Reaction(ROpt::Act(MOpt::Planet(*p)))).collect();
            let choice = Choice { actor: me, ask: Ask::Reaction(RAsk::Act(MAsk::Ouranos)), ctx: ctx(eff.codes().activation.titans_flagship, MSub::Ouranos), opts };
            let Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Planet(planet)))) = eff.ask(&choice) else { return Ok(()) };
            if !many.contains(&planet) {
                return Ok(());
            }
            planet
        }
    };
    // `replace_pds_with_flagship`: a damaged PDS is the one given up, then the first; the flagship arrives in the space area.
    let s = eff.state();
    let units = &mut s.board_mut().planet_mut(planet).units;
    let is_pds = |p: &ti4_engine_core::state::Piece| p.owner() == me && p.kind().data().base_type == UnitBaseType::Pds;
    let Some(index) = units.as_slice().iter().position(|p| is_pds(p) && p.sustained()).or_else(|| units.as_slice().iter().position(is_pds)) else { return Ok(()) };
    units.remove(index);
    fx::place_unit(s, me, system, None, flagship);
    Ok(())
}

/// Awaken, then Ouranos (the string's order in `titans::timing_abilities`, after Terragenesis).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(AWAKEN, AbilityKind::Ability, EventKind::SystemActivated, Relation::After, awaken).optional().when(awaken_when));
    t.add(Spec::new(OURANOS, AbilityKind::Unit, EventKind::SystemActivated, Relation::After, ouranos).optional().when(ouranos_when));
}
