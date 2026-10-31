//! The core twin of `action_cards::place_units_choosing` (Yin Spinner, the Naalu Thunder's Edge mech): "place up to `count` units of a base type from the reinforcements where the player chooses
//! among the spots" -- one spot is no decision (unless optional), all the units go to the spot chosen, nothing is asked or placed when the box holds none or no spot takes one.
//!
//! The spots are the planets the seat controls (not demilitarized) and, for Yin Spinner, the space areas holding one of its ships, in the string's order: the systems in id order, a system's
//! planets first, then its space. A space spot takes as many units as leave the fleet pool and the space-area capacity no worse than they are now (`action_cards::max_fit`, limits
//! `Respect`); a planet takes all. The form placed is the one the seat places (`placed_unit_id`: the upgrade it owns, its faction's unit, a faction override), a form a faction rule forbids
//! places none.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt, Subtype as USub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::Eff;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::fleet::{self, Arrival};
use ti4_engine_core::query::{board, laws, reach};
use ti4_engine_core::rules::production::probe;
use ti4_engine_core::state::CoreState;

use crate::cd_registry::yin::timing::placed_form_overridden;
use crate::common::scope;

/// Where the units may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Any planet the seat controls.
    ControlledPlanet,
    /// Any planet the seat controls or any space area holding one of its ships.
    ControlledPlanetOrShipSpace,
}

/// `placement_spots`: the spots in the string's order. Whether the box still holds the unit is not asked here.
#[must_use]
pub fn spots(s: &CoreState, who: Seat, target: Target) -> Vec<(System, Option<Planet>)> {
    let blanked = reach::blanked(s);
    let planets = board::controlled_planets(s, who);
    let mut found: Vec<(System, Option<Planet>)> = Vec::new();
    for system in System::ALL {
        for (home, planet) in planets.as_slice() {
            if *home == system && !laws::planet_is_demilitarized(s, blanked, *planet) {
                found.push((system, Some(*planet)));
            }
        }
        if target == Target::ControlledPlanetOrShipSpace && board::space_units(s, system).iter().any(|p| p.owner() == who && p.kind().data().ship) {
            found.push((system, None));
        }
    }
    found
}

/// `max_fit`: the largest `n <= wanted` units that leave the fleet pool and the capacity of the space area no worse than they are.
fn max_fit(s: &CoreState, k: &Kernel, who: Seat, system: System, planet: Option<Planet>, unit: Unit, wanted: usize) -> usize {
    if planet.is_some() {
        return wanted;
    }
    let limit = probe::fleet_limit(s, k, reach::blanked(s), who);
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(s, k, who, u);
    let before = fleet::standing(s, scope(k), who, system, limit, None, &half);
    (1..=wanted)
        .rev()
        .find(|n| {
            let after = fleet::standing(s, scope(k), who, system, limit, Some(Arrival { kind: unit, count: i64::try_from(*n).unwrap_or(i64::MAX), in_space: true }), &half);
            after.fleet_excess() <= before.fleet_excess() && after.capacity_excess <= before.capacity_excess
        })
        .unwrap_or(0)
}

/// The form `who` places for `base`, and how many of `count` the box lets it place (`supply::allowed`).
#[must_use]
pub fn form_and_room(s: &CoreState, k: &Kernel, who: Seat, base: UnitBaseType, count: usize) -> Option<(Unit, usize)> {
    let unit = placed_form_overridden(s, k, scope(k), who, base)?;
    let room = usize::try_from(fleet::remaining_all(s, scope(k), who, unit)).unwrap_or(0);
    Some((unit, room.min(count)))
}

/// What to place: `count` units of `base` for `who`, the question sourced from the interned `ability`.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub who: Seat,
    pub base: UnitBaseType,
    pub count: usize,
    pub target: Target,
    pub optional: bool,
    /// The place in `SilentCodes::g4` of the text naming the ability (the source of the question).
    pub ability: usize,
}

/// `place_units_choosing`: ask where (unless one spot decides it) and place; returns how many arrived.
pub fn choose(eff: &mut Eff<'_, '_>, p: Placement) -> usize {
    let (wanted, fitting) = {
        let (s, k) = eff.read();
        let Some((unit, wanted)) = form_and_room(s, k, p.who, p.base, p.count) else { return 0 };
        let fitting: Vec<(System, Option<Planet>, usize)> = spots(s, p.who, p.target)
            .into_iter()
            .filter_map(|(system, planet)| {
                let fit = max_fit(s, k, p.who, system, planet, unit, wanted);
                (fit > 0).then_some((system, planet, fit))
            })
            .collect();
        (wanted, fitting)
    };
    if wanted == 0 || fitting.is_empty() {
        return 0;
    }
    let (system, planet, fit) = match fitting.as_slice() {
        [only] if !p.optional => *only,
        _ => {
            let most = fitting.iter().map(|(_, _, fit)| *fit).max().unwrap_or(0);
            let mut opts: Opts = fitting.iter().map(|(system, planet, fit)| Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { system: *system, planet: *planet, fit: u8::try_from(*fit).unwrap_or(u8::MAX) })))).collect();
            if p.optional {
                opts.push(Opt::Decline);
            }
            let ability = eff.codes().silent.g4[p.ability];
            let choice = Choice {
                actor: p.who,
                ask: Ask::Reaction(RAsk::G(GAsk::U(UAsk::Place { base: p.base, fit: u8::try_from(most).unwrap_or(u8::MAX) }))),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Other(ability)),
                    subtype: Subtype::Reaction(RSub::G(GSub::U(USub::Place))),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { system, planet, .. })))) = eff.ask(&choice) else { return 0 };
            let Some(chosen) = fitting.iter().find(|(s, pl, _)| *s == system && *pl == planet) else { return 0 };
            *chosen
        }
    };
    let (s, k) = eff.state_and_kernel();
    let scope = scope(k);
    let form = placed_form_overridden(s, k, scope, p.who, p.base);
    fx::place_counted(s, k, scope, p.who, system, planet, form, fit)
}
