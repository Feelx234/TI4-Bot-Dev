//! The sources of the action menu that list cards and partners (package KM1, lane F): the relics, the exploration cards in the play area
//! (Enigmatic Device), the action cards of the hand and the salvage yard, and the transaction partners, read from the position and the hook
//! table alone. Only the latest rules version is supported (the game refuses an older one at load), so the version branches of the string
//! bodies are not here.
//!
//! The string functions this mirrors: `relics::available_actions`, `exploration::available_actions`, `action_cards::available_actions`
//! and `transactions::available_actions`. `ti4-engine`'s `menu_dual` builds the menu from these lists; `ti4-engine-core-bridge::
//! check_menu_sources` compares them with the string's over positions.

use crate::choice::turn::Opt;
use crate::generated::{ActionCard, ExploreType, Planet, Relic, System};
use crate::ids::Seat;
use crate::kernel::turn_codes::MenuGap;
use crate::kernel::Kernel;
use crate::query::economy::Spend;
use crate::query::{board, explore as qexplore, reach, relic as qrelic};
use crate::rules::action_cards as cards;
use crate::rules::production::strategy_cards::KernelHooks;
use crate::rules::production::{self as prod, Cx as ProdCx, Extras};
use crate::state::{CoreState, Slice};
use crate::tables::Scope;

/// The six resources of Enigmatic Device (`exploration::ENIGMATIC_DEVICE_COST`, `relics`).
const DEVICE_COST: i64 = 6;

pub(crate) fn resources(core: &CoreState, k: &Kernel, who: Seat) -> i64 {
    let cx = ProdCx::new(core, k, reach::scope_of(k), reach::blanked(core));
    prod::available(&cx, who, Spend::Resources, &Extras::default())
}

fn influence(core: &CoreState, k: &Kernel, who: Seat) -> i64 {
    let cx = ProdCx::new(core, k, reach::scope_of(k), reach::blanked(core));
    prod::available(&cx, who, Spend::Influence, &Extras::default())
}

/// Whether the seat could research something now (`technology::researchable_by_prerequisites` is not empty).
fn can_research_something(core: &CoreState, k: &Kernel, who: Seat) -> bool {
    let hooks = KernelHooks::new(core, k, who);
    !crate::query::tech::researchable_by_prerequisites(core, reach::scope_of(k), reach::blanked(core), who, &hooks).is_empty()
}

/// The planets Stellar Converter may destroy (`relics::stellar_converter_targets`): in a system next to one that holds one of the seat's
/// units with BOMBARDMENT, not a home planet, a legendary planet, Mecatol Rex or a planet already destroyed.
#[must_use]
pub fn stellar_targets(core: &CoreState, scope: Scope, who: Seat) -> Vec<(System, Planet)> {
    let mut reach_systems: Vec<System> = Vec::new();
    for (system, cell) in core.board().systems() {
        let bombards = cell.units.as_slice().iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.bombardment.is_some()))
            || board::planets_of(core, system)
                .any(|planet| board::planet_units(core, planet).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.bombardment.is_some())));
        if bombards && let Some(next) = board::neighbours(core, system) {
            reach_systems.extend(next.iter().filter_map(|i| System::ALL.get(i).copied()));
        }
    }
    reach_systems.sort_unstable();
    reach_systems.dedup();
    let mut out = Vec::new();
    for system in reach_systems {
        if !crate::query::leader_reads::has_entry(core, system) {
            continue;
        }
        for planet in board::planets_in(core, scope, system).as_slice() {
            let data = planet.data();
            if data.homeworld_of.is_some() || data.legendary || data.traits.contains(&crate::generated::PlanetTrait::Mr) {
                continue;
            }
            if core.board().planet(*planet).flags.has(crate::state::PlanetFlags::PURGED) {
                continue;
            }
            out.push((system, *planet));
        }
    }
    out
}

/// The relics' component actions (`relics::available_actions`): purge three fragments of a trait, then use a relic with an action.
#[must_use]
pub fn relics(core: &CoreState, k: &Kernel, who: Seat) -> Vec<Opt> {
    let mut out: Vec<Opt> = qexplore::purgeable(core, who).as_slice().iter().map(|t| Opt::Purge(*t)).collect();
    let holds = |r: Relic| qrelic::holds(core, who, r);
    let enigmatic = holds(Relic::EnigmaticDevice) && can_research_something(core, k, who) && resources(core, k, who) >= DEVICE_COST;
    let stellar = holds(Relic::StellarConverter) && core.loaded.has(Slice::Map) && !stellar_targets(core, reach::scope_of(k), who).is_empty();
    out.extend(qrelic::action_relics(core, who, enigmatic, stellar).as_slice().iter().map(|r| Opt::UseRelic(*r)));
    out
}

/// The exploration cards in the play area with an action (`exploration::available_actions`): Enigmatic Device, one option per card, when six
/// resources can be paid and a technology can be researched.
#[must_use]
pub fn exploration(core: &CoreState, k: &Kernel, who: Seat) -> Vec<Opt> {
    let Some(p) = core.players().by_seat(who) else { return Vec::new() };
    let cards: Vec<_> = p.cards.explore.as_slice().iter().copied().filter(|c| matches!(c, crate::generated::Explore::EnigmaticDevice1 | crate::generated::Explore::EnigmaticDevice2)).collect();
    if cards.is_empty() || resources(core, k, who) < DEVICE_COST || !can_research_something(core, k, who) {
        return Vec::new();
    }
    cards.into_iter().map(Opt::Device).collect()
}

/// The transaction partners (`transactions::available_actions`): none without a map. The partners are the trade island's
/// (`kernel::trade_env::open_partners_live`: Guild Ships, Trade Convoys, the Arbiters, the limits of 94.1).
#[must_use]
pub fn transactions(core: &CoreState, k: &Kernel, who: Seat) -> Vec<Opt> {
    if !core.loaded.has(Slice::Map) {
        return Vec::new();
    }
    crate::kernel::trade_env::open_partners_live(core, k, who)
        .as_slice()
        .iter()
        .filter_map(|&with| core.players().by_seat(with).map(|p| Opt::Open { with, faction: p.faction }))
        .collect()
}

/// `action_cards::cost_is_payable` for the six cards whose bill reads payment or research rules (`None`: the position is one the core cannot
/// judge, a counted gap).
fn bill_payable(core: &CoreState, k: &Kernel, who: Seat, card: ActionCard, goods: i64) -> Option<bool> {
    let scope = reach::scope_of(k);
    Some(match card {
        ActionCard::FocusedResearch => goods >= 4 && can_research_something(core, k, who),
        ActionCard::Impersonation => influence(core, k, who) >= 3,
        ActionCard::RevealPrototype => resources(core, k, who) >= 4 && reveal_prototype_has_upgrade(core, k, who),
        ActionCard::PirateFleet => resources(core, k, who) >= 3 && !pirate_systems(core, scope).is_empty(),
        ActionCard::MercenaryContract => goods >= 2 && !mercenary_planets(core, scope).is_empty(),
        ActionCard::Plagiarize => influence(core, k, who) >= 5 && plagiarizable(core, who)?,
        _ => return cards::cost_is_payable(core, who, card, goods),
    })
}

fn reveal_prototype_has_upgrade(core: &CoreState, k: &Kernel, who: Seat) -> bool {
    let hooks = KernelHooks::new(core, k, who);
    crate::query::tech::researchable_by_prerequisites(core, reach::scope_of(k), reach::blanked(core), who, &hooks).iter().any(crate::query::tech::is_unit_upgrade)
}

/// The home systems of the sources in scope.
fn home_systems(scope: Scope) -> Vec<System> {
    Planet::ALL.iter().filter(|p| p.data().homeworld_of.is_some() && scope.0 & crate::tables::source_bit(p.data().source) != 0).filter_map(|p| p.data().system).collect()
}

/// `mercenary_planets`: planets with a unit list (an empty one) off the home systems.
fn mercenary_planets(core: &CoreState, scope: Scope) -> Vec<(System, Planet)> {
    let homes = home_systems(scope);
    let mut out = Vec::new();
    for (planet, cell) in core.board().planets() {
        if !cell.flags.has(crate::state::PlanetFlags::UNITS_ENTRY) || !cell.units.is_empty() {
            continue;
        }
        let Some(system) = cell.placed_in.or(planet.data().system) else { continue };
        if !homes.contains(&system) && scope.0 & crate::tables::source_bit(planet.data().source) != 0 {
            out.push((system, planet));
        }
    }
    out
}

/// `pirate_systems_off_homes`: systems with a board entry off the home systems and without a unit of a seated player.
fn pirate_systems(core: &CoreState, scope: Scope) -> Vec<System> {
    let homes = home_systems(scope);
    System::ALL
        .iter()
        .copied()
        .filter(|s| crate::query::leader_reads::has_entry(core, *s) && !homes.contains(s) && board::space_units(core, *s).iter().all(|p| core.players().by_seat(p.owner()).is_none()))
        .collect()
}

/// `action_cards::neighbors`: the seats before and after in the seating order (the string's meaning of "neighbour" for the action cards).
fn seating_neighbours(core: &CoreState, who: Seat) -> Vec<Seat> {
    let order = core.seats().seating_order();
    let Some(i) = order.iter().position(|s| *s == who) else { return Vec::new() };
    let before = order[(i + order.len() - 1) % order.len()];
    let after = order[(i + 1) % order.len()];
    if before == after { vec![before] } else { vec![before, after] }
}

/// Plagiarize: a neighbour owns a non-faction technology the seat does not.
fn plagiarizable(core: &CoreState, who: Seat) -> Option<bool> {
    let mine = core.players().by_seat(who)?;
    Some(seating_neighbours(core, who).iter().any(|n| core.players().by_seat(*n).is_some_and(|p| p.tech.owned.iter().any(|t| !mine.tech.owned.contains(t) && t.data().faction.is_none()))))
}

/// The action cards the seat could play as component actions (`action_cards::available_actions`): the hand in hand order, then the salvage
/// yard. `goods` is the seat's spendable trade goods (`supply::potential_goods`).
///
/// # Errors
/// [`MenuGap::ActionCard`]: Signal Jamming, Seize Artifact or a card whose bill needs facts the core does not hold.
pub fn action_cards(core: &CoreState, k: &Kernel, who: Seat) -> Result<Vec<Opt>, MenuGap> {
    let Some(p) = core.players().by_seat(who) else { return Ok(Vec::new()) };
    let scope = reach::scope_of(k);
    let blanked = reach::blanked(core);
    let goods = p.econ.trade_goods.max(0) as i64 + k.hooks.direct.keleres_spendable_commodities(core, who) + {
        if k.hooks.direct.keleres_agent_could_grant(core, who) { i64::from(p.econ.commodities.max(0)) } else { 0 }
    };
    let barred = k.hooks.economy.action_cards_forbidden(core, who) || crate::rules::agenda::political_secret_bars(core, who) || crate::query::laws::censure_bars(core, blanked, who);
    let playable = |card: ActionCard| -> Result<bool, MenuGap> {
        if !cards::is_component_action(card) {
            return Ok(false);
        }
        if barred {
            return Ok(false);
        }
        if card == ActionCard::SignalJamming {
            return Err(MenuGap::ActionCard);
        }
        if !bill_payable(core, k, who, card, goods).ok_or(MenuGap::ActionCard)? {
            return Ok(false);
        }
        match card {
            ActionCard::SeizeArtifact => Ok(seize_artifact_target(core, who)),
            _ => cards::has_legal_target(core, blanked, scope, who, card).ok_or(MenuGap::ActionCard),
        }
    };
    let mut out = Vec::new();
    for (index, card) in p.cards.hand.as_slice().iter().enumerate() {
        if playable(*card)? {
            out.push(Opt::Play { index: u8::try_from(index).map_err(|_| MenuGap::ActionCard)?, card: *card });
        }
    }
    for card in qexplore::salvaged(core, who) {
        if playable(*card)? {
            out.push(Opt::Salvaged(*card));
        }
    }
    Ok(out)
}

/// Seize Artifact: a neighbour holds a relic fragment.
fn seize_artifact_target(core: &CoreState, who: Seat) -> bool {
    seating_neighbours(core, who).iter().any(|n| core.players().by_seat(*n).is_some_and(|p| ExploreType::ALL.iter().any(|t| p.cards.fragments.get(*t).unwrap_or(0) > 0)))
}
