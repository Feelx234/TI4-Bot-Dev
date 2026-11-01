//! The leader component actions of the action menu (`Game::apply_choice`, the `component|leader|` arm, over `perform_leader_action` and
//! `leaders::use_leader`): the window of each leader, its facts computed from the core, the card exhausted (an agent) or purged (a hero)
//! when it resolved, then the verdict. A leader whose window is not routed yet is the counted gap [`MenuGap::Unrouted`] (named by the
//! option's key).
//!
//! Routed: Ggrocuto Rinn (Xxcha agent, an ACTION), Carth of Golden Sands (Hacan agent) and Nekro Malleon (Nekro agent), both printed "during
//! the action phase" (a free use), Letani Ospha (Arborec agent), Jace X. (Sol hero), Darktalon Treilla (Letnev hero, stays in play until
//! the round ends), The Helmsman (L1Z1X hero), Gurno Aggero (Saar hero), Ahk-Syl Siven (Nomad hero), Ul the Progenitor (Titans hero) and Xxekir
//! Grom (the Thunder's Edge card set's Xxcha hero). A copy of an agent through Ssruu (`Opt::Borrowed`) runs the same body and exhausts Ssruu.
//! The rest of the agents and heroes are still lent (docs/KM4_RESUME.md lists them with what each needs).

use super::component::{announce_staged, conclude_announced, verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::generated::{Leader, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx_strategy;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::query::{board, reach};
use crate::rules::leader_windows::{ArmageddonRelay, HacanAgent, Helmsman, LetaniOspha, NekroMalleon, XxchaAgent, XxchaHeroTe};
use crate::state::{PlainKind, PlainRec};

/// The unit roster of the table's play facts (the forms a seat places).
fn roster(sx: &Sx<'_>) -> Option<&'static crate::query::tech::UnitRoster> {
    sx.table.facts.plays.as_ref().and_then(|p| p.roster)
}

/// Whether the leader's use (`borrowed`: the copy through Ssruu) has a native body: [`run`] never ends it in the counted gap
/// [`MenuGap::Unrouted`]. The shadow oracle lends exactly the others.
#[must_use]
pub const fn routed(leader: Leader, borrowed: bool) -> bool {
    if borrowed {
        return matches!(leader, Leader::GgrocutoRinn | Leader::CarthOfGoldenSands | Leader::LetaniOspha);
    }
    matches!(
        leader,
        Leader::GgrocutoRinn
            | Leader::CarthOfGoldenSands
            | Leader::NekroMalleon
            | Leader::LetaniOspha
            | Leader::JaceX4thAirLegion
            | Leader::AhkSylSiven
            | Leader::UlTheProgenitor
            | Leader::DarktalonTreilla
            | Leader::TheHelmsman
            | Leader::GurnoAggero
            | Leader::XxekirGromTe
    )
}

/// Run the use of `leader` chosen as option `key`; `borrowed`: the copy of the agent that Ssruu holds.
pub fn run(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool) -> Flow {
    if borrowed {
        // `leaders::use_leader_text`: the same body, then Ssruu (not the copied agent) exhausts. Only agents can be copied.
        return match leader {
            Leader::GgrocutoRinn => xxcha_agent(sx, who, key, leader, true),
            Leader::CarthOfGoldenSands => carth(sx, who, key, leader, true),
            Leader::LetaniOspha => letani_ospha(sx, who, key, leader, true),
            _ => unrouted(sx, key),
        };
    }
    match leader {
        Leader::GgrocutoRinn => xxcha_agent(sx, who, key, leader, false),
        Leader::CarthOfGoldenSands => carth(sx, who, key, leader, false),
        Leader::NekroMalleon => nekro_malleon(sx, who, key),
        Leader::LetaniOspha => letani_ospha(sx, who, key, leader, false),
        Leader::JaceX4thAirLegion => {
            let used = crate::rules::leaders::sol_hero(sx.state, who);
            hero(sx, who, key, leader, used)
        }
        Leader::AhkSylSiven => {
            let used = crate::rules::leaders::nomad_hero(sx.state, who);
            hero(sx, who, key, leader, used)
        }
        Leader::UlTheProgenitor => {
            let used = crate::rules::leaders::titans_hero(sx.state);
            hero(sx, who, key, leader, used)
        }
        Leader::DarktalonTreilla => darktalon(sx, who, key),
        Leader::TheHelmsman => helmsman(sx, who, key),
        Leader::GurnoAggero => saar_hero(sx, who, key),
        Leader::XxekirGromTe => xxcha_hero_te(sx, who, key),
        _ => unrouted(sx, key),
    }
}

/// The leader has no native body yet: the counted gap names the option.
fn unrouted(sx: &mut Sx<'_>, key: u32) -> Flow {
    gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(5, key)))
}

/// A used agent exhausts (a copy through Ssruu exhausts Ssruu, the card that carries the text); a hero is purged by its caller.
fn exhaust_agent(sx: &mut Sx<'_>, who: Seat, leader: Leader, borrowed: bool) {
    let card = if borrowed { Leader::Ssruu } else { leader };
    let _ = fx_strategy::exhaust_ready_agent(sx.state, sx.kernel, who, card);
}

/// The end of an agent printed "ACTION:" (takes the turn's action): exhaust it when it resolved, then the shared tail.
fn agent_action(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool, used: bool) -> Flow {
    if used {
        exhaust_agent(sx, who, leader, borrowed);
    }
    conclude_announced(sx, who, key, verdict(used))
}

/// The end of a free use ("during the action phase" is not an ACTION, OP-07): the use does not take the turn, so there is no Extreme Duress and
/// no end of the action; the string says `LEADER_ABILITY_RESOLVED` and the seat is asked for its action again.
fn agent_free(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool, used: bool) -> Flow {
    if used {
        exhaust_agent(sx, who, leader, borrowed);
    }
    if announce_staged(sx) == Flow::Halt {
        return Flow::Halt;
    }
    if used { // mutants:skip: the menu offers a free use only when it can resolve, so the failed arm is the defensive twin of the string's
        sx.plain(PlainRec::bare(PlainKind::LeaderAbilityResolved));
    } else {
        sx.plain(PlainRec::bare(PlainKind::ComponentActionFailed));
        sx.state.pending_mut().note_failed_key(key);
    }
    Flow::Done
}

/// The end of a hero's ACTION that asked nothing (or whose window has run): purged when it resolved (51.9, 51.10), then the shared tail.
fn hero(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, used: bool) -> Flow {
    if used {
        crate::rules::leaders::purge(sx.state, who, leader);
    }
    conclude_announced(sx, who, key, verdict(used))
}

/// Ggrocuto Rinn: ready an exhausted planet (asked when there are several), and remove an infantry from a planet next to one of the seat's. An
/// ACTION: the card exhausts, Extreme Duress settles, and a resolved use ends the action. The string arm has a map in every live game.
fn xxcha_agent(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool) -> Flow {
    let mut window = XxchaAgent::new(who, true); // mutants:skip: every live game has a map (the string arm: `galaxy` is `Some`)
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    agent_action(sx, who, key, leader, borrowed, window.used)
}

/// Carth of Golden Sands: gain 2 commodities or replenish another player's. "During the action phase" is not an ACTION (OP-07).
fn carth(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool) -> Flow {
    // The replenish is announced as the string does (a Cabal's `COMMODITIES_REPLENISHED`, a Trade Agreement's gain): the source is the interned
    // `trade_agreement` of the table's non-supply gains.
    let mut window = HacanAgent::with_source(who, sx.table.codes.not_from_supply[3]); // mutants:skip: index 3 is `trade_agreement` (pinned by the shadow of cabal 5p seed 22; the test table interns no texts)
    if run_window(sx, &mut window) == Flow::Halt || super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    agent_free(sx, who, key, leader, borrowed, window.used)
}

/// Nekro Malleon: a player who could pay discards an action card or spends a command token to gain 2 trade goods. Printed "during the action phase"
/// (a free use). The string declines at once when the card is not the user's own readied one (a copy through Ssruu is not offered, so the window never
/// meets it).
fn nekro_malleon(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = NekroMalleon::new(who, sx.table.codes.turn.nekroagent);
    if run_window(sx, &mut window) == Flow::Halt || super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    agent_free(sx, who, key, Leader::NekroMalleon, false, window.used)
}

/// Letani Ospha (Arborec agent): a ship is replaced by one that costs up to 2 more. `forms` is the unit each seat places per ship base type.
fn letani_ospha(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let bases = [UnitBaseType::Carrier, UnitBaseType::Cruiser, UnitBaseType::Destroyer, UnitBaseType::Dreadnought, UnitBaseType::Flagship, UnitBaseType::Warsun];
    let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    let forms: Vec<(Seat, Unit)> = seats
        .into_iter()
        .flat_map(|seat| bases.into_iter().filter_map(move |base| Some((seat, base))).collect::<Vec<_>>())
        .filter_map(|(seat, base)| fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, seat, base).map(|unit| (seat, unit)))
        .collect();
    let mut window = LetaniOspha::new(who, forms);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    agent_action(sx, who, key, leader, borrowed, window.used)
}

/// Darktalon Treilla: fleet supply is unlimited until the end of this round. She is not purged by the use (`end_of_round` purges her); the
/// string `exhaust` moves only a readied card, so an unlocked hero stays as she is.
fn darktalon(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let used = crate::rules::leaders::activate_letnev_hero(sx.state, who);
    conclude_announced(sx, who, key, verdict(used))
}

/// The Helmsman: the flagship and the dreadnoughts are placed in one system holding no rival ships.
fn helmsman(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = Helmsman::new(who, true);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::TheHelmsman, window.used)
}

/// Gurno Aggero: destroy the other players' infantry and fighters in a system next to one of the seat's space docks.
fn saar_hero(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = ArmageddonRelay::new(who, true);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::GurnoAggero, window.used)
}

/// Xxekir Grom (the Thunder's Edge card set's Xxcha hero): up to four PDS or mechs on controlled planets, each planet readied. The supply left is
/// asked of each form with a want of one (`supply::allowed(.., 1)`), as the string does.
fn xxcha_hero_te(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let planets: Vec<(System, Planet)> = board::controlled_planets(sx.state, who).as_slice().to_vec();
    let form = |base: UnitBaseType| -> Option<(Unit, usize)> {
        let unit = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, base)?;
        let allowed = !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, unit) && crate::query::fleet::remaining_all(sx.state, scope, who, unit) > 0; // mutants:skip: `supply::allowed(.., 1)`; the box-empty and forbidden edges are the string's (the menu offers the hero only with a PDS in the box)
        Some((unit, usize::from(allowed)))
    };
    let (pds, mech) = (form(UnitBaseType::Pds), form(UnitBaseType::Mech));
    let mut window = XxchaHeroTe::new(who, &planets, pds, mech);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::XxekirGromTe, window.used)
}
