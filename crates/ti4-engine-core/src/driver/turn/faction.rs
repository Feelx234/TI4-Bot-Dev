//! The faction component actions of the action menu (`faction_abilities::perform_component` and the modules' `perform_component`): each
//! code of [`crate::kernel::hooks::components`] is routed to the rule or the window that runs it, the facts computed from the core. An
//! action whose window is not native yet is the counted gap [`MenuGap::Unrouted`] (named by the option's key).

use super::component::{conclude_announced, verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::choice::turn::{ComponentAction, Opt};
use crate::generated::UnitBaseType;
use crate::ids::Seat;
use crate::kernel::hooks::components as c;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::kernel::fx_strategy;
use crate::query::reach;
use crate::rules::relic_windows::{Fabrication, FabMode};

/// The unit roster of the table's play facts (the forms a seat places).
fn roster(sx: &Sx<'_>) -> Option<&'static crate::query::tech::UnitRoster> {
    sx.table.facts.plays.as_ref().and_then(|p| p.roster)
}

/// Run the faction component action `action` chosen as option `key`.
pub fn run(sx: &mut Sx<'_>, who: Seat, key: u32, action: ComponentAction) -> Flow {
    match action.code {
        c::LAZAX_GATE_FOLDING => lazax_gate_folding(sx, who, key),
        c::ORBITAL_DROP => orbital_drop(sx, who, key),
        c::NUCLEUS => star_forge(sx, who, key, true),
        c::STAR_FORGE => star_forge(sx, who, key, false),
        c::INFERNO => inferno(sx, who, key),
        c::FIRES => fires(sx, who, key),
        c::PRODUCTION_BIOMES => production_biomes(sx, who, key),
        c::TRADE_CONVOYS => trade_convoys(sx, who, key),
        c::BLOOD_PACT => pact(sx, who, key, crate::generated::PromissoryNote::BloodPact),
        c::DARK_PACT => pact(sx, who, key, crate::generated::PromissoryNote::DarkPact),
        c::PROMISE_OF_PROTECTION => promise_of_protection(sx, who, key),
        c::STALL_TACTICS => stall_tactics(sx, who, key),
        c::MAGEON_IMPLANTS => mageon_implants(sx, who, key, action.arg),
        c::FABRICATION_RELIC => fabrication(sx, who, key, FabMode::Relic { bmf: false }),
        c::FABRICATION_TOKEN => fabrication(sx, who, key, FabMode::Token),
        c::BLACK_MARKET_FORGERY => fabrication(sx, who, key, FabMode::Relic { bmf: true }),
        c::WORMHOLE_GENERATOR => wormhole_generator(sx, who, key),
        c::C4R_PLACE => c4r_place(sx, who, key),
        c::C4R_REPAIR => c4r_repair(sx, who, key),
        c::C4R_DRAW => c4r_draw(sx, who, key),
        c::TERRAFORM => terraform(sx, who, key, action.arg),
        c::VORTEX => vortex(sx, who, key),
        // mutants:skip (the key default): every option on the menu has a key
        _ => gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(super::action::gap_code(&crate::driver::dispatch::Dispatch::Faction(action)), Opt::Faction(action).key().unwrap_or(0)))), // mutants:skip (the key default): every option on the menu has a key
    }
}

/// Lazax Gate Folding (Winnu): the infantry goes to Mecatol Rex (`winnu::lazax_gate_folding_string`).
fn lazax_gate_folding(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let form = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let mecatol = crate::query::objectives::mecatol_on(sx.state);
    let done = crate::rules::faction_component::lazax_gate_folding(sx.state, scope, who, mecatol, form);
    conclude_announced(sx, who, key, verdict(done))
}

/// Orbital Drop (Sol; `faction_abilities::orbital_drop`): the core window asks the planet, spends the token, places the infantry and offers the mech.
/// It asks through the tape; a table without a link to the payment window is the counted gap `NoLink`.
fn orbital_drop(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::generated::Unit;
    use crate::rules::faction_component::{OrbitalDrop, OrbitalFacts};
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let infantry = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let mech = scope.unit(Unit::ZSThunderboltM2).map(|_| Unit::ZSThunderboltM2);
    let can_pay = crate::driver::menu_cards::resources(sx.state, sx.kernel, who) >= 3;
    let facts = OrbitalFacts { infantry, mech, can_pay, scope, source: sx.table.codes.turn.orbital_drop };
    let mut window = OrbitalDrop::new(who, facts);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    // The token the window spent is announced at once by the string (`hooks_economy::emit`), and only while a seated faction has a module.
    if super::component::announce_events(sx) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// The form a seat places for a base type (`factions::muaat::placement_facts`).
fn unit_form(sx: &Sx<'_>, who: Seat, base: UnitBaseType) -> Option<crate::generated::Unit> {
    let roster = roster(sx)?;
    fx_strategy::placed_unit(sx.state, sx.kernel, reach::scope_of(sx.kernel), roster, who, base)
}

/// `fleet::limit`, or the unlimited bill while Darktalon Treilla's round lasts.
fn fleet_limit(sx: &Sx<'_>, who: Seat) -> i64 {
    crate::rules::production::probe::fleet_limit(sx.state, sx.kernel, crate::query::reach::blanked(sx.state), who)
}

/// Star Forge (`nucleus`: false) and The Nucleus (`nucleus`: true): 2 fighters or 1 destroyer at a war sun, then the Ember Colossus placements. The
/// systems next to each war sun are the map's own adjacency (empty while no galaxy is loaded, as the string without one).
fn star_forge(sx: &mut Sx<'_>, who: Seat, key: u32, nucleus: bool) -> Flow {
    use crate::rules::muaat_component::{ForgeFacts, Mode, StarForge};
    if roster(sx).is_none() {
        return gap_flow(sx, TurnGap::NoRoster);
    }
    let suns = StarForge::war_sun_systems(sx.state, who);
    let near = suns
        .into_iter()
        .map(|system| {
            let next_to = crate::query::board::neighbours(sx.state, system).map(|set| set.iter().filter_map(crate::generated::System::from_index).collect()).unwrap_or_default();
            (system, next_to)
        })
        .collect();
    let facts = ForgeFacts {
        scope: reach::scope_of(sx.kernel),
        fighter: unit_form(sx, who, UnitBaseType::Fighter),
        destroyer: unit_form(sx, who, UnitBaseType::Destroyer),
        infantry: unit_form(sx, who, UnitBaseType::Infantry),
        fleet_limit: fleet_limit(sx, who),
        near,
        source: sx.table.codes.turn.star_forge,
        mech: sx.table.codes.turn.muaat_mech,
    };
    let mut window = StarForge::new(who, if nucleus { Mode::Nucleus } else { Mode::StarForge }, facts);
    if run_window(sx, &mut window) == Flow::Halt || super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// The Inferno: a cruiser placed in the system of the Muaat flagship, the strategy token spent after it.
fn inferno(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::rules::muaat_component::{inferno, InfernoFacts};
    if roster(sx).is_none() {
        return gap_flow(sx, TurnGap::NoRoster);
    }
    let facts = InfernoFacts { scope: reach::scope_of(sx.kernel), cruiser: unit_form(sx, who, UnitBaseType::Cruiser), fleet_limit: fleet_limit(sx, who), source: sx.table.codes.turn.inferno };
    let done = inferno(sx.state, sx.kernel, who, &facts);
    if super::component::announce_events(sx) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(done))
}

/// Fires of the Gashlai (the Muaat promissory note): the first Muaat seat in seating order is the owner.
fn fires(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let owner = sx.state.players().iter().find(|p| p.faction == crate::generated::Faction::TheEmbersOfMuaat).map(|p| p.seat);
    let done = owner.is_some_and(|owner| crate::rules::muaat_component::fires_of_the_gashlai(sx.state, sx.kernel, reach::scope_of(sx.kernel), roster, who, owner));
    conclude_announced(sx, who, key, verdict(done))
}

/// Production Biomes (Hacan): spend a strategy token, another player gains 2 trade goods (the window asks who).
fn production_biomes(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = crate::rules::technology_windows::ProductionBiomes::new(who, sx.table.codes.turn.production_biomes);
    if run_window(sx, &mut window) == Flow::Halt || super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// Trade Convoys (the Hacan promissory note): the first one in the hand (by the text of its faction id) goes faceup.
fn trade_convoys(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let done = crate::rules::trade::play_convoys(sx.state, who, |faction| faction.index());
    conclude_announced(sx, who, key, verdict(done))
}

/// Blood Pact and Dark Pact (the Empyrean promissory notes): "place this card faceup in your play area".
fn pact(sx: &mut Sx<'_>, who: Seat, key: u32, note: crate::generated::PromissoryNote) -> Flow {
    let note = crate::state::NoteKey { note, faction: crate::generated::Faction::TheEmpyrean };
    let done = crate::rules::trade::play_action_note(sx.state, who, note);
    conclude_announced(sx, who, key, verdict(done))
}

/// Promise of Protection (the Mentak promissory note): the first copy in the hand goes faceup.
fn promise_of_protection(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let note = crate::query::promissory::action_notes_in_hand(sx.state, who).into_iter().find(|n| n.note == crate::generated::PromissoryNote::PromiseOfProtection);
    let done = note.is_some_and(|note| crate::rules::trade::play_action_note(sx.state, who, note));
    conclude_announced(sx, who, key, verdict(done))
}

/// Naaz Fabrication (a relic for 2 fragments, or a command token for 1) and Black Market Forgery (the relic for 2 fragments and the held note).
fn fabrication(sx: &mut Sx<'_>, who: Seat, key: u32, mode: FabMode) -> Flow {
    let source = if matches!(mode, FabMode::Relic { bmf: true }) { sx.table.codes.turn.bmf } else { sx.table.codes.turn.fabrication };
    let mut window = Fabrication::new(who, mode, source);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// Yssaril's Stall Tactics: discard an action card, then optionally place a mech.
fn stall_tactics(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let mech = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Mech)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let turn = sx.table.codes.turn;
    let mut window = crate::rules::promissory_windows::StallTactics::new(who, mech, scope, turn.stall_tactics, turn.yssaril_mech);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// Yssaril's Mageon Implants: look at the hand of the player `arg` names and take a card.
fn mageon_implants(sx: &mut Sx<'_>, who: Seat, key: u32, target: u16) -> Flow {
    let Ok(target) = u8::try_from(target) else { return gap_flow(sx, TurnGap::ReplayDiverged) };
    let mut window = crate::rules::promissory_windows::MageonImplants::new(who, Seat(target), true); // mutants:skip: the hand limit is the printed one (the string bails when a faction changes it)
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// Wormhole Generator (Ghost): place or move a Creuss wormhole token into a system that holds a planet the seat controls or a non-home system holding
/// no other player's ship. Each token (alpha, then beta) into each such system it is not in, the systems in map order; none without a map.
fn wormhole_generator(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::generated::Wormhole;
    let scope = reach::scope_of(sx.kernel);
    let mut systems: Vec<(crate::hex::Hex, crate::generated::System)> = Vec::new();
    if let Some(galaxy) = sx.state.galaxy() {
        systems = galaxy
            .in_play()
            .iter()
            .filter_map(crate::generated::System::from_index)
            .filter(|s| !galaxy.is_hyperlane(*s))
            .filter_map(|s| galaxy.coord_of(s).map(|hex| (hex, s)))
            .collect();
        systems.sort_unstable_by_key(|(hex, _)| *hex);
    }
    let others_ships = |system: crate::generated::System| {
        crate::query::board::space_units(sx.state, system).iter().any(|p| p.owner() != who && scope.unit(p.kind()).is_some_and(|d| d.ship))
    };
    let open: Vec<crate::generated::System> = systems
        .iter()
        .map(|(_, system)| *system)
        .filter(|system| crate::query::board::controls_a_planet_in(sx.state, *system, who) || (!system.data().home_system && !others_ships(*system)))
        .collect();
    let mut destinations = Vec::new();
    for kind in [Wormhole::Alpha, Wormhole::Beta] {
        for system in &open {
            if sx.state.explore().wormhole_tokens.get(&kind) != Some(system) {
                destinations.push((kind, *system));
            }
        }
    }
    let mut window = crate::rules::technology_windows::WormholeGenerator::new(who, destinations);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// Nekro's ???_ERROR_ERROR_???, the PDS: place 1 PDS on a planet the seat controls (the planet is asked when there are several), then exhaust the card.
fn c4r_place(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::rules::promissory_windows::{PlaceUnits, Then};
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let pds = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Pds)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let mut window = PlaceUnits::new(who, pds, 1, None, false, scope, crate::state::Code(0), Then::C4rPds); // mutants:skip: a capped placement names the card, not this source
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.placed > 0)) // mutants:skip: the menu lists the action only when a PDS has a planet to go on, so `placed` is never 0
}

/// Nekro's ???_ERROR_ERROR_???, the repair: every damaged unit of the seat is repaired and the card is exhausted.
fn c4r_repair(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let done = crate::rules::faction_component::c4r_repair(sx.state, who);
    conclude_announced(sx, who, key, verdict(done))
}

/// Nekro's ???_ERROR_ERROR_???, the draw: discard an action card, exhaust the card, draw one (the hand limit is asked after the draw).
fn c4r_draw(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = crate::rules::promissory_windows::C4rDraw::new(who, sx.state.meta().rules_at_least(OFFICIAL_ACTION_DECK));
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    conclude_announced(sx, who, key, verdict(window.done))
}

/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const OFFICIAL_ACTION_DECK: u32 = 30;

/// Terraform (the Titans promissory note): attach it to the planet the option names.
fn terraform(sx: &mut Sx<'_>, who: Seat, key: u32, arg: u16) -> Flow {
    let Some(planet) = crate::generated::Planet::from_index(usize::from(arg)) else { return gap_flow(sx, TurnGap::ReplayDiverged) };
    // (The menu lists no home planet; the rule reads the flag again.)
    let home = planet.data().homeworld_of.is_some() || sx.state.players().iter().any(|p| p.home_planets.as_slice().contains(&planet)); // mutants:skip: the menu lists no home planet
    let done = crate::rules::faction_component::terraform(sx.state, who, planet, home);
    conclude_announced(sx, who, key, verdict(done))
}

/// Cabal's Vortex: capture a non-structure unit type of another player next to one of the seat's space docks (the type is asked when there are several).
fn vortex(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let neutral = sx.table.facts.plays.as_ref().and_then(|p| p.shared.neutral);
    let mut window = crate::rules::technology_windows::Vortex::new(who, sx.state.galaxy().is_some(), neutral, reach::scope_of(sx.kernel));
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    conclude_announced(sx, who, key, verdict(window.done))
}
