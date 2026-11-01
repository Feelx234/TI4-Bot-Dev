//! The heroes of the action menu that read the position for their facts (`hero_dual.rs` is the string adapter's side of the same windows): UNIT.DSGN.FLAYESH
//! (Nekro), Kyver, Blade and Key (Yssaril), It Feeds on Carrion (Cabal) and Hesh and Prit (Naaz). Each is a window of `rules::hero_windows`; the facts it
//! takes are computed here from the core. The systems and the strategy cards are listed in the order of the core's tables (the string lists them in the order of
//! the id texts: the same wherever the ids are numbered in step, the shadow of `native_play` is the witness).

use super::component::conclude_announced;
use super::inline::{gap_flow, run as run_window, Flow};
use crate::generated::{Ability, Leader, StrategyCard, System, TechType, Technology, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::SeeingLimits;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::fx;
use crate::query::{board, reach};
use crate::rules::hero_windows::{ArgentHero, CabalHero, Devourable, EmpyreanHero, KyverHero, MigrationFacts, NaazHero, NekroHero, Stranded};
use crate::state::Piece;

/// A hero's end: purged when it resolved, then the shared tail.
fn finish(sx: &mut Sx<'_>, who: Seat, key: u32, leader: Leader, used: bool) -> Flow {
    if used {
        crate::rules::leaders::purge(sx.state, who, leader);
    }
    conclude_announced(sx, who, key, super::component::verdict(used))
}

/// The colour a technology specialty names (unit upgrades have none).
fn is_colour(t: TechType) -> bool {
    matches!(t, TechType::Biotic | TechType::Cybernetic | TechType::Propulsion | TechType::Warfare)
}

/// Whether the seat has a unit in the system (space or any planet).
fn holds_units(sx: &Sx<'_>, who: Seat, system: System) -> bool {
    board::space_units(sx.state, system).iter().any(|p| p.owner() == who) || board::planets_of(sx.state, system).any(|planet| board::planet_units(sx.state, planet).iter().any(|p| p.owner() == who))
}

/// UNIT.DSGN.FLAYESH: the planets that carry a technology specialty in a system holding a unit of the seat, with the trade goods each would give (its resources
/// and influence now) and the technologies of its colours the seat may gain.
pub fn flayesh(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let scope = reach::scope_of(sx.kernel);
    let blanked = reach::blanked(sx.state);
    let Some(p) = sx.state.players().by_seat(who) else { return gap_flow(sx, TurnGap::ReplayDiverged) };
    let faction = p.faction;
    let active = crate::query::tech::active(scope);
    let Some(alias_order) = sx.table.facts.plays.as_ref().map(|plays| plays.tech_order) else { return gap_flow(sx, TurnGap::NoRoster) };
    let mut targets: Vec<Devourable> = Vec::new();
    for system in System::ALL.iter().copied().filter(|s| crate::query::leader_reads::has_entry(sx.state, *s)) {
        if !holds_units(sx, who, system) {
            continue;
        }
        for planet in board::planets_in(sx.state, scope, system).as_slice().iter().copied() {
            let colours: Vec<TechType> = crate::query::tech::specialties_now(sx.state, scope, planet).as_slice().iter().copied().filter(|t| is_colour(*t)).collect();
            if colours.is_empty() {
                continue;
            }
            let goods = crate::query::economy::value_now(sx.state, scope, blanked, planet, crate::query::economy::Spend::Resources, 0)
                + crate::query::economy::value_now(sx.state, scope, blanked, planet, crate::query::economy::Spend::Influence, 0);
            let owned = sx.state.players().by_seat(who).map(|p| p.tech.owned);
            let mut techs: Vec<Technology> = Technology::ALL
                .iter()
                .copied()
                .filter(|t| active.contains(*t))
                .filter(|t| owned.is_none_or(|o| !o.contains(*t)))
                .filter(|t| !crate::query::tech::is_unit_upgrade(*t))
                .filter(|t| t.data().types.first().is_some_and(|c| colours.contains(c)))
                .filter(|t| t.data().faction.is_none_or(|tag| crate::query::tech::tag_is(tag, faction)))
                .collect();
            // The string lists the active deck in the order of the alias text.
            techs.sort_by(|a, b| alias_order(*a, *b));
            targets.push(Devourable { system, planet, goods: i32::try_from(goods).unwrap_or(i32::MAX), techs });
        }
    }
    let mut window = NekroHero::new(who, sx.table.codes.turn.nekrohero, targets);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    // The ground forces that fell are announced before the goods (the string's order).
    if super::component::announce_staged(sx) == Flow::Halt || super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    finish(sx, who, key, Leader::UNITDSGNFLAYESH, window.used)
}

/// Kyver, Blade and Key: each other player shows an action card, and the seat takes it or has that player discard three. A seat whose faction lifts the hand
/// limit (Yssaril's Crafty) is not asked to return cards afterwards.
pub fn kyver(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let limited = !sx.state.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Crafty));
    let mut window = KyverHero::new(who, limited);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    finish(sx, who, key, Leader::KyverBladeAndKey, window.used)
}

/// It Feeds on Carrion: every other player's non-fighter ship in or next to a system holding a dimensional tear rolls a die; on a 1 to 3 it is captured. The
/// owners and systems whose cargo the captures strand enforce their capacity afterwards.
pub fn carrion(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let scope = reach::scope_of(sx.kernel);
    let is_tear = |p: &Piece| matches!(p.kind(), crate::generated::Unit::DimensionalTearI | crate::generated::Unit::DimensionalTearII);
    let tears: Vec<System> = System::ALL
        .iter()
        .copied()
        .filter(|s| board::space_units(sx.state, *s).iter().any(is_tear) || board::planets_of(sx.state, *s).any(|planet| board::planet_units(sx.state, planet).iter().any(is_tear)))
        .collect();
    let mut reach_systems: Vec<System> = Vec::new();
    for tear in &tears {
        if let Some(next) = board::neighbours(sx.state, *tear) {
            reach_systems.extend(next.iter().filter_map(System::from_index));
        }
        reach_systems.push(*tear);
    }
    reach_systems.sort_unstable();
    reach_systems.dedup();
    let mut targets: Vec<(System, Piece)> = Vec::new();
    for system in reach_systems {
        if !crate::query::leader_reads::has_entry(sx.state, system) {
            continue;
        }
        for piece in board::space_units(sx.state, system) {
            let data = scope.unit(piece.kind());
            if piece.owner() != who && data.is_some_and(|d| d.ship && d.base_type != UnitBaseType::Fighter) {
                targets.push((system, *piece));
            }
        }
    }
    let mut pairs: Vec<(Seat, System)> = targets.iter().map(|(system, piece)| (piece.owner(), *system)).collect();
    pairs.sort_unstable();
    pairs.dedup();
    let stranded: Vec<Stranded> = pairs
        .into_iter()
        .map(|(owner, system)| {
            let limits = crate::driver::windows::tactical::limits(sx.state, sx.kernel, owner, system);
            Stranded { owner, system, limits: SeeingLimits { enforce: limits.enforce, shown: limits.shown, halves: limits.halves } }
        })
        .collect();
    let mut window = CabalHero::new(who, true, targets, stranded);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    finish(sx, who, key, Leader::ItFeedsOnCarrion, window.used)
}

/// The cards on the board a strategy hero may name: held and not exhausted, or still unclaimed.
fn board_cards(sx: &Sx<'_>) -> Vec<StrategyCard> {
    let mut cards: Vec<StrategyCard> = sx
        .state
        .players()
        .iter()
        .flat_map(|p| p.strat.as_slice().iter().copied().filter(|c| !p.strat_exhausted.contains(*c)).collect::<Vec<_>>())
        .chain(sx.state.strategy().unclaimed.as_slice().iter().copied())
        .collect();
    cards.sort_unstable();
    cards.dedup();
    cards
}

/// The facts a hero that performs strategy cards reads: the base of the first card, with what only some cards need (the Imperial primary's objectives, the
/// Thunder's Edge Warfare and Construction systems) taken from their own.
pub fn perform_facts(sx: &Sx<'_>, who: Seat, cards: &[StrategyCard]) -> Option<crate::kernel::fx_perform::Facts> {
    let first = *cards.first()?;
    let mut facts = crate::driver::windows::strategic::facts::perform_facts(sx, who, first);
    for card in cards {
        let more = crate::driver::windows::strategic::facts::perform_facts(sx, who, *card);
        facts.imperial = facts.imperial.or(more.imperial);
        facts.free = facts.free.or(more.free);
        facts.te_uses = facts.te_uses.or(more.te_uses);
    }
    Some(facts)
}

/// Hesh and Prit: a relic, then the secondary of up to two cards on the board without spending command tokens. A secondary that makes units at the home
/// system is announced where the core finished it (after the events the secondaries before it staged).
pub fn hesh_and_prit(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let cards = board_cards(sx);
    let Some(facts) = perform_facts(sx, who, &cards) else { return finish(sx, who, key, Leader::HeshAndPrit, false) };
    let mut window = NaazHero::new(who, facts, cards);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::StrategyAbility);
    }
    if let (Some(made), Some(home)) = (window.home_made.clone(), sx.state.players().by_seat(who).and_then(|p| p.home_system)) {
        let all = fx::drain_staged(sx.state);
        let at = window.home_staged.min(all.len());
        let (before, after) = all.split_at(at);
        for rec in before {
            fx::stage(sx.state, rec.clone());
        }
        crate::driver::windows::strategic::perform::announce_made(sx, who, home, &made);
        for rec in after {
            fx::stage(sx.state, rec.clone());
        }
    }
    if super::component::announce_events(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    finish(sx, who, key, Leader::HeshAndPrit, window.used)
}

/// Mirik Aun Sissiri (Argent hero): ships move to systems holding the seat's command token and no other player's ships. The window takes the systems of
/// the board (those with an entry), the ones on the map (a coordinate or a wormhole) and the ones the seat's movement rules let it enter, which is read
/// for the systems holding its token.
pub fn flock_migration(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(galaxy) = sx.state.galaxy() else { return finish(sx, who, key, Leader::MirikAunSissiri, false) };
    let Some(reach_rules) = reach::SeatReach::new(sx.state, sx.kernel, who, true) else { return gap_flow(sx, TurnGap::NoRoster) };
    let mut order = Vec::new();
    let mut on_map = Vec::new();
    let mut enterable = Vec::new();
    for system in System::ALL.iter().copied().filter(|s| crate::query::leader_reads::has_entry(sx.state, *s)) {
        order.push(system);
        if galaxy.coord_of(system).is_some() || galaxy.wormhole_kinds(system) != 0 {
            on_map.push(system);
        }
        if sx.state.board().sys(system).cmd.contains(who) && reach_rules.rules_for(sx.state, sx.kernel, system).can_enter(system) {
            enterable.push(system);
        }
    }
    let mut window = ArgentHero::new(who, MigrationFacts { order, on_map, enterable });
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    finish(sx, who, key, Leader::MirikAunSissiri, window.moved)
}

/// Conservator Procyon (Empyrean hero): a frontier token in every system that holds no planet (a space station is not one) and has none, then each token in
/// a system holding one of the seat's ships is explored. The window walks the systems in the order given, which here is the order of the core's tables.
pub fn conservator_procyon(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let scope = reach::scope_of(sx.kernel);
    let Some(galaxy) = sx.state.galaxy() else { return finish(sx, who, key, Leader::ConservatorProcyon, false) };
    let in_play = *galaxy.in_play();
    let placed: Vec<System> = System::ALL
        .iter()
        .copied()
        .filter(|s| in_play.contains(s.index()) && !galaxy.is_hyperlane(*s))
        .filter(|s| s.data().planets.iter().filter(|p| scope.0 & crate::tables::source_bit(p.data().source) != 0).all(|p| crate::query::objectives::is_space_station(*p)))
        .collect();
    let order: Vec<System> = System::ALL.to_vec();
    let mut window = EmpyreanHero::new(who, placed, order);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    finish(sx, who, key, Leader::ConservatorProcyon, window.used)
}

/// Every strategy card in the game, held by a seat or unclaimed (Mathis Mathinus may name an exhausted one).
fn every_card(sx: &Sx<'_>) -> Vec<StrategyCard> {
    let mut cards: Vec<StrategyCard> = sx.state.players().iter().flat_map(|p| p.strat.as_slice().to_vec()).chain(sx.state.strategy().unclaimed.as_slice().iter().copied()).collect();
    cards.sort_unstable();
    cards.dedup();
    cards
}

/// Mathis Mathinus (Winnu hero): the seat chooses a strategy card and performs its primary; each other player may then follow its secondary. The hero stays in
/// play (`LeaderStrategy`) until the follower window is over, which is the strategic lane's (`complete_leader_strategy` purges it and tells the table). The
/// Thunder's Edge Warfare primary leads to a free tactical action, which is the tactical lane's (`TurnGap::FreeTactical`).
pub fn mathis_mathinus(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let cards = every_card(sx);
    let Some(facts) = perform_facts(sx, who, &cards) else { return finish(sx, who, key, Leader::MathisMathinus, false) };
    let mut window = crate::rules::hero_windows::WinnuHero::new(who, facts, cards);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::StrategyAbility);
    }
    let Some(card) = window.chosen else { return conclude_failed(sx, who, key) };
    if let Some((system, seats)) = window.tokens.clone() {
        crate::driver::windows::strategic::perform::place_tokens(sx, system, seats.as_slice());
    }
    if window.scored.is_some() {
        let unlocking = crate::driver::turn::start::unlockable(sx, who, true);
        crate::rules::leaders::unlock(sx.state, who, unlocking.as_slice());
    }
    if card.data().kind == crate::generated::StrategyCardKind::Imperial {
        fx::finish_if_won(sx.state);
    }
    if let Some((system, made)) = window.produced.clone().filter(|(_, made)| !made.as_slice().is_empty()) {
        crate::driver::windows::strategic::perform::announce_made(sx, who, system, &made);
    }
    if window.free.is_some() {
        return gap_flow(sx, TurnGap::FreeTactical);
    }
    // An ACTION: what the body staged is announced (the events at once, as the string's adapter applies them) and Extreme Duress settles now; the action ends
    // when the follower window does.
    if super::component::announce_events(sx) == Flow::Halt || super::component::announce_staged(sx) == Flow::Halt || super::component::settle_extreme_duress(sx, who, false) == Flow::Halt {
        return Flow::Halt;
    }
    sx.state.pending_mut().open(crate::state::Window::LeaderStrategy { player: who, leader: Leader::MathisMathinus, card });
    crate::driver::windows::strategic::select_leader_strategy_followers(sx.state);
    Flow::Done
}

/// The hero chose no card: the action failed and is withheld.
fn conclude_failed(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    conclude_announced(sx, who, key, super::component::Verdict::Failed { withhold: true })
}
