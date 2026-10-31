//! The faction component actions of the action menu (package KM1, lane F): Orbital Drop, Production Biomes and Trade Convoys
//! (`faction_abilities::component_actions`), then the faction modules' own in the order of the string's `MODULES`
//! (`factions::component_actions`, `factions::mapped_component_actions`). Read from the position and the hook table alone.
//!
//! The actions that need a Thunder's Edge breakthrough (Psychospore, Deorbit Barrage, Vaults of the Heir, Deepgloom Executable) are out of
//! scope: no position of the game offers them. Where a listing needs a rule the core does not hold (the PDS placement of Nekro's
//! `???_ERROR_ERROR_???`, Terraform's targets, Cabal's Vortex) the source is the counted gap [`MenuGap::Faction`] for exactly the seats that
//! could be offered it, never a silent omission.

use crate::choice::turn::Opt;
use crate::generated::{Faction, Planet, PromissoryNote, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::hooks::components as c;
use crate::kernel::hooks::sites::ComponentAction;
use crate::kernel::turn_codes::MenuGap;
use crate::kernel::{fx, fx_strategy, Kernel};
use crate::query::tech::UnitRoster;
use crate::query::{board, explore as qexplore, promissory as qpromissory, reach};
use crate::state::CoreState;

fn action(module: Faction, code: u16, arg: u16) -> Opt {
    Opt::Faction(ComponentAction { module, code, arg })
}

fn faction_of(core: &CoreState, who: Seat) -> Option<Faction> {
    core.players().by_seat(who).map(|p| p.faction)
}

/// The notes of `who` that are in hand with an ACTION that places them. The string walks them in the order of the note id text
/// (`alias:faction`); the typed order differs only between aliases, and the aliases that matter here (`blood_pact`, `dark_pact`) keep it.
fn action_notes(core: &CoreState, who: Seat) -> Vec<crate::state::NoteKey> {
    qpromissory::action_notes_in_hand(core, who)
}

/// `who` holds a foreign copy of `note` (any, faceup or not) whose owner is not its own faction.
fn holds_foreign(core: &CoreState, who: Seat, note: PromissoryNote) -> bool {
    qpromissory::held_foreign(core, who, note) > 0
}

fn war_sun_systems(core: &CoreState, scope: crate::tables::Scope, who: Seat) -> Vec<crate::generated::System> {
    core.board()
        .systems()
        .filter(|(_, cell)| cell.units.as_slice().iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Warsun)))
        .map(|(s, _)| s)
        .collect()
}

/// Muaat's Star Forge: a war sun's system where 2 fighters or 1 destroyer fit and are in the box.
fn forge_possible(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> bool {
    let scope = reach::scope_of(k);
    let limit = crate::rules::production::probe::fleet_limit(core, k, reach::blanked(core), who);
    war_sun_systems(core, scope, who).into_iter().any(|system| {
        [(UnitBaseType::Fighter, 2), (UnitBaseType::Destroyer, 1)].into_iter().any(|(base, wanted)| {
            fx_strategy::placed_unit(core, k, scope, roster, who, base)
                .is_some_and(|unit| fx::box_has(core, k, scope, who, unit) && fx::max_fit(core, k, scope, who, system, true, unit, wanted, limit) > 0)
        })
    })
}

fn muaat(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat, out: &mut Vec<Opt>) {
    let scope = reach::scope_of(k);
    let Some(p) = core.players().by_seat(who) else { return };
    let is_muaat = p.faction == Faction::TheEmbersOfMuaat;
    let strategic = p.econ.strategic_tokens > 0;
    let forge = forge_possible(core, k, roster, who);
    if qexplore::legendary_available(core, who, Planet::Avernus) && forge {
        out.push(action(Faction::TheEmbersOfMuaat, c::NUCLEUS, 0));
    }
    if is_muaat && strategic && forge {
        out.push(action(Faction::TheEmbersOfMuaat, c::STAR_FORGE, 0));
    }
    if strategic && fx::box_has(core, k, scope, who, Unit::CruiserI) {
        // The first system (in system order) holding the seat's flagship; the cruiser must fit there.
        let flagship = core.board().systems().find(|(_, cell)| cell.units.as_slice().iter().any(|u| u.owner() == who && u.kind() == Unit::TheInferno)).map(|(s, _)| s);
        if let Some(system) = flagship {
            let limit = crate::rules::production::probe::fleet_limit(core, k, reach::blanked(core), who);
            if fx::max_fit(core, k, scope, who, system, true, Unit::CruiserI, 1, limit) > 0 {
                out.push(action(Faction::TheEmbersOfMuaat, c::INFERNO, 0));
            }
        }
    }
    // Fires of the Gashlai: the note of the Muaat seat, held by someone else who has no war sun technology, while the Muaat have a fleet token.
    if holds_foreign(core, who, PromissoryNote::FiresOfTheGashlai)
        && !is_muaat
        && !p.tech.owned.contains(Technology::WarSun)
        && core.players().iter().any(|m| m.faction == Faction::TheEmbersOfMuaat && m.econ.fleet_tokens > 0)
    {
        out.push(action(Faction::TheEmbersOfMuaat, c::FIRES, 0));
    }
}

/// The fragments of one trait (`naaz::fragments`).
fn fragments(core: &CoreState, who: Seat, kind: crate::generated::ExploreType) -> i32 {
    core.players().by_seat(who).and_then(|p| p.cards.fragments.get(kind)).map_or(0, i32::from)
}

/// `naaz::purgeable_types(.., 2)`: the traits with two fragments counting the frontier ones.
fn purgeable_two(core: &CoreState, who: Seat) -> bool {
    use crate::generated::ExploreType as E;
    let frontier = fragments(core, who, E::Frontier);
    [E::Cultural, E::Hazardous, E::Industrial].into_iter().any(|kind| fragments(core, who, kind) + frontier >= 2)
}

fn naaz(core: &CoreState, who: Seat, out: &mut Vec<Opt>) {
    let relic_left = !core.decks().relic.is_empty();
    if faction_of(core, who) == Some(Faction::TheNaazRokhaAlliance) {
        if relic_left && purgeable_two(core, who) {
            out.push(action(Faction::TheNaazRokhaAlliance, c::FABRICATION_RELIC, 0));
        }
        let any_fragment = core.players().by_seat(who).is_some_and(|p| crate::generated::ExploreType::ALL.iter().any(|t| p.cards.fragments.get(*t).is_some_and(|n| n > 0)));
        if any_fragment && fx::tokens_in_reinforcements(core, who) > 0 {
            out.push(action(Faction::TheNaazRokhaAlliance, c::FABRICATION_TOKEN, 0));
        }
    }
    if relic_left && holds_foreign(core, who, PromissoryNote::BlackMarketForgery) && purgeable_two(core, who) {
        out.push(action(Faction::TheNaazRokhaAlliance, c::BLACK_MARKET_FORGERY, 0));
    }
}

/// The component actions of the faction modules and the three loose abilities for `who`.
///
/// # Errors
/// [`MenuGap::Faction`] for a seat that could be offered an action whose listing the core does not hold.
pub fn faction(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> Result<Vec<Opt>, MenuGap> {
    let Some(p) = core.players().by_seat(who) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    let strategic = p.econ.strategic_tokens > 0;
    // `faction_abilities::component_actions`
    if p.faction == Faction::TheFederationOfSol && strategic && board::controlled_count(core, who) > 0 {
        out.push(action(Faction::TheFederationOfSol, c::ORBITAL_DROP, 0));
    }
    if reach::technology_text_ready(core, who, Technology::ProductionBiomes) && strategic && core.players().iter().any(|o| o.seat != who) {
        out.push(action(Faction::TheEmiratesOfHacan, c::PRODUCTION_BIOMES, 0));
    }
    let notes = action_notes(core, who);
    if notes.iter().any(|n| n.note == PromissoryNote::TradeConvoys) {
        out.push(action(Faction::TheEmiratesOfHacan, c::TRADE_CONVOYS, 0));
    }
    // The modules, in the order of `factions::MODULES`: cabal (mapped), empyrean, ghost, mentak, muaat, naaz, nekro, titans, winnu, yssaril.
    // Mapped actions come after every plain one (`turn_options` lists the Faction source, then the Mapped one): collected apart.
    let mapped: Vec<Opt> = Vec::new();
    for n in &notes {
        match n.note {
            PromissoryNote::BloodPact => out.push(action(Faction::TheEmpyrean, c::BLOOD_PACT, 0)),
            PromissoryNote::DarkPact => out.push(action(Faction::TheEmpyrean, c::DARK_PACT, 0)),
            _ => {}
        }
    }
    if reach::technology_text_ready(core, who, Technology::WormholeGenerator) {
        out.push(action(Faction::TheGhostsOfCreuss, c::WORMHOLE_GENERATOR, 0));
    }
    if notes.iter().any(|n| n.note == PromissoryNote::PromiseOfProtection) {
        out.push(action(Faction::TheMentakCoalition, c::PROMISE_OF_PROTECTION, 0));
    }
    muaat(core, k, roster, who, &mut out);
    naaz(core, who, &mut out);
    // Nekro's ???_ERROR_ERROR_???: the PDS placement reads the placement spots of the seat, which the core does not hold.
    if reach::technology_text_ready(core, who, Technology::ERRORERROR) {
        return Err(MenuGap::Faction(Faction::TheNekroVirus));
    }
    // Terraform: its targets read the home planets of every seat.
    if holds_foreign(core, who, PromissoryNote::Terraform) {
        return Err(MenuGap::Faction(Faction::TheTitansOfUl));
    }
    // Lazax Gate Folding.
    if reach::technology_text_ready(core, who, Technology::LazaxGateFolding)
        && (board::controls(core, who, Planet::MecatolRex) || mecatol_planet_held(core, who))
        && fx::box_has(core, k, reach::scope_of(k), who, Unit::InfantryI)
    {
        out.push(action(Faction::TheWinnu, c::LAZAX_GATE_FOLDING, 0));
    }
    // Yssaril: Stall Tactics, then Mageon Implants.
    if p.faction == Faction::TheYssarilTribes && !p.cards.hand.is_empty() {
        out.push(action(Faction::TheYssarilTribes, c::STALL_TACTICS, 0));
    }
    if reach::technology_text_ready(core, who, Technology::MageonImplants) {
        for other in core.players().iter().filter(|o| o.seat != who && !o.cards.hand.is_empty()) {
            out.push(action(Faction::TheYssarilTribes, c::MAGEON_IMPLANTS, u16::from(other.seat.0)));
        }
    }
    // Cabal's Vortex is a mapped action: it needs the targets over the map.
    if faction_of(core, who) == Some(Faction::TheVuilraithCabal) && reach::technology_text_ready(core, who, Technology::Vortex) {
        return Err(MenuGap::Faction(Faction::TheVuilraithCabal));
    }
    out.extend(mapped);
    Ok(out)
}

/// The seat controls a planet in the Mecatol Rex system (`winnu::mecatol_planets` is not empty).
fn mecatol_planet_held(core: &CoreState, who: Seat) -> bool {
    let system = crate::query::objectives::mecatol_in_galaxy(core);
    board::controls_a_planet_in(core, system, who)
}
