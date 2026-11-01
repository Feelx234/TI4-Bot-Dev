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
use crate::rules::leader_production::{ArborecHero, Umbat};
use crate::rules::leader_windows::{ArmageddonRelay, HacanAgent, Helmsman, LetaniOspha, NekroMalleon, RiftwalkerMeian, RinHero, SwapFacts, XxchaAgent, XxchaHeroTe, ZeuAction};
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
        return matches!(leader, Leader::GgrocutoRinn | Leader::CarthOfGoldenSands | Leader::LetaniOspha | Leader::Umbat);
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
            | Leader::Umbat
            | Leader::LetaniMiasmiala
            | Leader::RinTheMastersLegacy
            | Leader::RiftwalkerMeian
            | Leader::UNITDSGNFLAYESH
            | Leader::KyverBladeAndKey
            | Leader::ItFeedsOnCarrion
            | Leader::HeshAndPrit
            | Leader::MirikAunSissiri
            | Leader::ConservatorProcyon
            | Leader::MathisMathinus
            | Leader::Zeu
            | Leader::AiroShirAur
            | Leader::DannelOfTheTenth
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
            Leader::Umbat => umbat(sx, who, key),
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
        Leader::Umbat => umbat(sx, who, key),
        Leader::LetaniMiasmiala => miasmiala(sx, who, key),
        Leader::RinTheMastersLegacy => rin(sx, who, key),
        Leader::RiftwalkerMeian => riftwalker_meian(sx, who, key),
        Leader::UNITDSGNFLAYESH => super::hero::flayesh(sx, who, key),
        Leader::KyverBladeAndKey => super::hero::kyver(sx, who, key),
        Leader::ItFeedsOnCarrion => super::hero::carrion(sx, who, key),
        Leader::HeshAndPrit => super::hero::hesh_and_prit(sx, who, key),
        Leader::MirikAunSissiri => super::hero::flock_migration(sx, who, key),
        Leader::ConservatorProcyon => super::hero::conservator_procyon(sx, who, key),
        Leader::MathisMathinus => super::hero::mathis_mathinus(sx, who, key),
        Leader::Zeu => zeu(sx, who, key),
        Leader::AiroShirAur => super::hero::airo_shir_aur(sx, who, key),
        Leader::DannelOfTheTenth => super::hero::dannel(sx, who, key),
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

/// Z'eu (Naalu agent, an ACTION): the seat chooses a system it could activate that holds neither a home system nor its own command token (the
/// question stands with the decline alone when there is none), the card exhausts, Extreme Duress settles, and the free tactical action begins there
/// (`Game::perform_zeu`; the action's own close ends the turn). A declined question fails the action, which is withheld for the rest of the turn.
fn zeu(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let leave = crate::rules::tactical::holds_leave(sx.state, sx.kernel, who);
    let homes = |s: System| sx.state.players().iter().any(|p| p.home_system == Some(s));
    let systems: Vec<System> = crate::query::transit::activatable(sx.state, sx.kernel, who, leave)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !board::has_token(sx.state, *s, who) && !homes(*s))
        .collect();
    let mut window = ZeuAction::new(who, systems);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let Some(system) = window.chosen else {
        sx.plain(PlainRec::bare(PlainKind::ComponentActionFailed));
        sx.state.pending_mut().note_failed_key(key);
        return Flow::Done;
    };
    exhaust_agent(sx, who, Leader::Zeu, false);
    if super::component::settle_extreme_duress(sx, who, false) == Flow::Halt {
        return Flow::Halt;
    }
    sx.plain(PlainRec::bare(PlainKind::ComponentActionResolved));
    crate::driver::windows::tactical::begin_free(sx, who, system);
    Flow::Done
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

/// Umbat (Muaat agent): a player chosen by the holder produces up to 2 units of cost 4 or less in a system with a war sun or the flagship. Each
/// player's use has the facts of a production by an ability (`probe::ability_facts`); the end of the use is the string adapter's tail
/// ([`super::produce::finish_uses`]). The agent is spent once a player was chosen; a copy through Ssruu spends Ssruu (the window knows which text
/// the seat acts on).
fn umbat(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    let mut facts = Vec::new();
    for seat in seats {
        for system in Umbat::candidates(sx.state, seat) {
            let Some(use_facts) = crate::rules::production::probe::ability_facts(sx.state, sx.kernel, roster, seat, system, 2, Some(4)) else { return gap_flow(sx, TurnGap::NoRoster) }; // mutants:skip: the ceiling is 4 and no unit costs exactly 5 (halves: 1, 2, 4, 6, 8, 16, 20, 24), so 4 and 5 offer the same units
            facts.push(use_facts);
        }
    }
    let mut window = Umbat::new(who, facts);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    if let (Some(target), Some(made)) = (window.target, window.made.clone()) {
        super::produce::finish_uses(sx, target, &[made]);
    }
    if super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    agent_action(sx, who, key, Leader::Umbat, window.copied, window.used)
}

/// Letani Miasmiala (Arborec hero): one use of PRODUCTION by an ability in every system holding one of the seat's ground forces, with no limit
/// on the number of units. Purged when some unit was made.
fn miasmiala(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let mut facts = Vec::new();
    for system in ArborecHero::ground_force_systems(sx.state, who) {
        let Some(use_facts) = crate::rules::production::probe::ability_facts(sx.state, sx.kernel, roster, who, system, HERO_LIMIT, None) else { return gap_flow(sx, TurnGap::NoRoster) };
        facts.push(use_facts);
    }
    let mut window = ArborecHero::new(who, facts);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    super::produce::finish_uses(sx, who, &window.made);
    if super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::LetaniMiasmiala, window.used())
}

/// The number of units one use of Letani Miasmiala may make: none is set by the card; the purse and the box bound it (`arborec::HERO_LIMIT`).
const HERO_LIMIT: i64 = 1000;

/// Rin, the Master's Legacy (Jol-Nar hero): each non-unit-upgrade technology may be swapped for one of its colour. The replacements are listed in the order of
/// the corpus file and the technologies held in the order of their alias text (the string's `BTreeSet` order): both orders are the adapter's tables.
fn rin(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(plays) = sx.table.facts.plays.as_ref() else { return gap_flow(sx, TurnGap::NoRoster) };
    let (alias_order, corpus) = (plays.tech_order, sx.table.codes.agenda.tech_corpus);
    let mut order = crate::generated::Technology::ALL.to_vec();
    order.sort_by(|a, b| corpus(*a, *b));
    let mut by_alias = crate::generated::Technology::ALL.to_vec();
    by_alias.sort_by(|a, b| alias_order(*a, *b));
    let mut window = RinHero::new(who, order, by_alias);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::RinTheMastersLegacy, window.used)
}

/// Riftwalker Meian (Ghost hero): swap the positions of two systems that hold a wormhole or the seat's units. The systems the card may not name are the
/// two Creuss systems, the Wormhole Nexus and the Fracture's; the wormholes are the map's (printed and tokens).
fn riftwalker_meian(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::bits::SysSet;
    use crate::generated::System;
    let scope = reach::scope_of(sx.kernel);
    let (mut allowed, mut wormholes) = (SysSet::EMPTY, SysSet::EMPTY);
    let has_map = sx.state.galaxy().is_some();
    if let Some(galaxy) = sx.state.galaxy() {
        for system in System::ALL {
            let excluded = matches!(system, System::CreussGateCreuss | System::CreussCreuss | System::MalliceLockedNexus | System::MalliceNexus)
                || scope.system(system).is_some_and(|d| d.tile_back == Some(crate::generated::TileBack::Fracture));
            if excluded {
                continue;
            }
            allowed.insert(system.index());
            if galaxy.wormhole_kinds(system) != 0 {
                wormholes.insert(system.index());
            }
        }
    }
    let mut window = RiftwalkerMeian::new(who, has_map, SwapFacts { allowed, wormholes });
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    hero(sx, who, key, Leader::RiftwalkerMeian, window.used)
}
