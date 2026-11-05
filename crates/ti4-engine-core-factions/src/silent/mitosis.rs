//! arborec: Mitosis (`factions::arborec::mitosis`): "At the start of the status phase: place 1 infantry from your reinforcements on any planet you control."
//! With the Letani Behemoth the player may instead replace 1 of their infantry with 1 mech. Mandatory; the one question (where, or which replacement) is asked
//! before anything changes and a single possible placement is not a decision. Registered for every seat (`ability:*:mitosis:STATUS_PHASE_BEGAN:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Ability, Faction, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::{board, laws, reach};
use ti4_engine_core::state::{CoreState, EventKind, Piece};
use ti4_engine_core::tables::Scope;

use crate::cd_registry::yin::timing::placed_form;

/// Mitosis (`ability:*:mitosis:STATUS_PHASE_BEGAN:after`).
pub const MITOSIS: Slot = Slot(40_007);

/// One way to resolve Mitosis.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Move {
    replace: bool,
    system: System,
    planet: Option<Planet>,
}

fn has_mitosis(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Mitosis))
}

fn scope(k: &Kernel) -> Scope {
    Scope(u32::from(k.sources.0))
}

/// `box_has`: the box still holds one of the unit the seat places for `base`, and no effect forbids placing it.
fn box_form(s: &CoreState, k: &Kernel, who: Seat, base: UnitBaseType) -> Option<Unit> {
    let unit = placed_form(s, scope(k), who, base)?;
    fx::box_has(s, k, scope(k), who, unit).then_some(unit)
}

fn infantry_in(pieces: &[Piece], who: Seat) -> Option<usize> {
    pieces.iter().position(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry)
}

/// `mitosis_moves`, in the string's order: the placements on the controlled planets (system, then planet), then, per system, the replacements on its planets and in
/// its space area.
fn moves(s: &CoreState, k: &Kernel, who: Seat) -> Vec<Move> {
    let mut out = Vec::new();
    if box_form(s, k, who, UnitBaseType::Infantry).is_some() {
        let blanked = reach::blanked(s);
        for (system, planet) in board::controlled_planets(s, who).as_slice() {
            if !laws::planet_is_demilitarized(s, blanked, *planet) {
                out.push(Move { replace: false, system: *system, planet: Some(*planet) });
            }
        }
    }
    if box_form(s, k, who, UnitBaseType::Mech).is_some() {
        // (An infantry of `who` stands only where `who` has a unit at all.)
        for system in board::systems_of(board::systems_with_units_anywhere(s, who)) {
            let mut planets: Vec<Planet> = board::planets_of(s, system).collect();
            planets.sort_unstable();
            for planet in planets {
                if infantry_in(board::planet_units(s, planet), who).is_some() {
                    out.push(Move { replace: true, system, planet: Some(planet) });
                }
            }
            if infantry_in(board::space_units(s, system), who).is_some() {
                out.push(Move { replace: true, system, planet: None });
            }
        }
    }
    out
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    has_mitosis(c.state, me) && !moves(c.state, c.kernel, me).is_empty()
}

fn mitosis(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !has_mitosis(eff.state(), me) {
        return Ok(());
    }
    let found = {
        let (s, k) = eff.read();
        moves(s, k, me)
    };
    // The replacement is a "may": with no placement to fall back on it is declinable.
    let only_replacements = found.iter().all(|m| m.replace);
    let chosen = match found.as_slice() {
        [] => return Ok(()),
        [only] if !only.replace => *only,
        _ => {
            let mut opts: Opts = found.iter().map(|m| Opt::Reaction(ROpt::G(GOpt::Mitosis { replace: m.replace, system: m.system, planet: m.planet }))).collect();
            if only_replacements {
                opts.push(Opt::Decline);
            }
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::Mitosis)),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Other(eff.codes().silent.mitosis)),
                    subtype: Subtype::Reaction(RSub::G(GSub::Mitosis)),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Mitosis { replace, system, planet }))) = eff.ask(&choice) else { return Ok(()) };
            let picked = Move { replace, system, planet };
            if !found.contains(&picked) {
                return Ok(());
            }
            picked
        }
    };
    let sc = scope(eff.kernel());
    let (s, k) = eff.state_and_kernel();
    if chosen.replace {
        replace_infantry(s, k, me, chosen);
    } else {
        let form = placed_form(s, sc, me, UnitBaseType::Infantry);
        let _ = fx::place_counted(s, k, sc, me, chosen.system, chosen.planet, form, 1);
    }
    Ok(())
}

/// `replace_unit(.., "infantry", "mech")`: the first infantry of the seat on the spot goes, a mech of the seat's own form takes its place at the end of the list.
fn replace_infantry(s: &mut CoreState, k: &Kernel, who: Seat, at: Move) {
    let sc = scope(k);
    let Some(mech) = placed_form(s, sc, who, UnitBaseType::Mech) else { return };
    if k.hooks.economy.effect_placement_forbidden(s, k, who, mech) || ti4_engine_core::query::fleet::remaining_all(s, sc, who, mech) <= 0 {
        return;
    }
    let units = match at.planet {
        Some(planet) => &mut s.board_mut().planet_mut(planet).units,
        None => &mut s.board_mut().sys_mut(at.system).units,
    };
    let Some(index) = infantry_in(units.as_slice(), who) else { return };
    units.remove(index);
    units.push(Piece::new(mech, who));
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MITOSIS, AbilityKind::Ability, EventKind::StatusPhaseBegan, Relation::After, mitosis).applies(Applies::Faction(Faction::TheArborec)).when(condition));
}
