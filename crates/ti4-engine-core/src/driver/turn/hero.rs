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
// mutants:skip: a planet's specialty is always one of the four colours (the unit-upgrade type is never a specialty), so the test is a guard
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
pub fn perform_facts(sx: &Sx<'_>, who: Seat, cards: &[StrategyCard]) -> crate::kernel::fx_perform::Facts {
    // (No card on the board: the base of any card serves, the window has nothing to perform.)
    let first = cards.first().copied().unwrap_or(StrategyCard::ALL[0]); // mutants:skip: the base facts of any card serve when none stands on the board
    let mut facts = crate::driver::windows::strategic::facts::perform_facts(sx, who, first);
    for card in cards {
        let more = crate::driver::windows::strategic::facts::perform_facts(sx, who, *card);
        facts.imperial = facts.imperial.or(more.imperial);
        facts.free = facts.free.or(more.free);
        facts.te_uses = facts.te_uses.or(more.te_uses);
    }
    facts
}

/// Hesh and Prit: a relic, then the secondary of up to two cards on the board without spending command tokens. A secondary that makes units at the home
/// system is announced where the core finished it (after the events the secondaries before it staged).
pub fn hesh_and_prit(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let cards = board_cards(sx);
    let facts = perform_facts(sx, who, &cards);
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
    let Some(galaxy) = sx.state.galaxy() else { return finish(sx, who, key, Leader::MirikAunSissiri, false) }; // mutants:skip: every live game has a map (the card is not offered without one)
    let Some(reach_rules) = reach::SeatReach::new(sx.state, sx.kernel, who, true) else { return gap_flow(sx, TurnGap::NoRoster) }; // mutants:skip: the cards in play that change movement are read by `apply_movement_effects`; none stands in the tests and the effect only widens `enterable`, which the window narrows to token systems
    let mut order = Vec::new();
    let mut on_map = Vec::new();
    let mut enterable = Vec::new();
    for system in System::ALL.iter().copied().filter(|s| crate::query::leader_reads::has_entry(sx.state, *s)) {
        order.push(system);
        if galaxy.coord_of(system).is_some() || galaxy.wormhole_kinds(system) != 0 { // mutants:skip: a system on the map has a coordinate; only an off-grid wormhole system (the open Nexus) is on the map by its wormhole
            on_map.push(system);
        }
        if sx.state.board().sys(system).cmd.contains(who) && reach_rules.rules_for(sx.state, sx.kernel, system).can_enter(system) { // mutants:skip: the window keeps only the systems holding the token among the enterable ones, so the token test here is a repeat
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
    let Some(galaxy) = sx.state.galaxy() else { return finish(sx, who, key, Leader::ConservatorProcyon, false) }; // mutants:skip: every live game has a map
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
/// Thunder's Edge Warfare primary leads to a free tactical action ([`crate::driver::windows::tactical::begin_free`]), after which the followers are selected.
pub fn mathis_mathinus(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let cards = every_card(sx);
    let facts = perform_facts(sx, who, &cards);
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
    if window.scored.is_some() { // mutants:skip: the same announcements as `strategic::perform::announce` (tested with the strategic arms); the Imperial primary needs a scoreable objective
        let unlocking = crate::driver::turn::start::unlockable(sx, who, true); // mutants:skip: as above: the same announcement as the strategic arm
        crate::rules::leaders::unlock(sx.state, who, unlocking.as_slice());
    }
    if card.data().kind == crate::generated::StrategyCardKind::Imperial { // mutants:skip: as above
        fx::finish_if_won(sx.state);
    }
    if let Some((system, made)) = window.produced.clone().filter(|(_, made)| !made.as_slice().is_empty()) {
        crate::driver::windows::strategic::perform::announce_made(sx, who, system, &made);
    }
    // Thunder's Edge Warfare: the free tactical action begins now (the string activates the system inside the hero's arm, before the staged events are
    // told); the follower window waits for its end (`strategic::after`), so the hero's strategy is open but its followers are not selected yet.
    let free = window.free;
    if let Some(system) = free {
        crate::driver::windows::tactical::begin_free(sx, who, system);
    }
    // An ACTION: what the body staged is announced (the events at once, as the string's adapter applies them) and Extreme Duress settles now; the action ends
    // when the follower window does.
    if super::component::announce_events(sx) == Flow::Halt || super::component::announce_staged(sx) == Flow::Halt || super::component::settle_extreme_duress(sx, who, false) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the job does after the halt
        return Flow::Halt;
    }
    sx.state.pending_mut().open(crate::state::Window::LeaderStrategy { player: who, leader: Leader::MathisMathinus, card });
    if free.is_none() {
        crate::driver::windows::strategic::select_leader_strategy_followers(sx.state);
    }
    Flow::Done
}

/// The hero chose no card: the action failed and is withheld.
// mutants:skip: a window with cards always ends on an answer; the verdict is the defensive twin of the string's
fn conclude_failed(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    conclude_announced(sx, who, key, super::component::Verdict::Failed { withhold: true })
}

/// Airo Shir Aur (Mahact hero): the `(origin, destination)` pairs, in board order: the seat has ships in the origin's space area, the destination is
/// adjacent and holds exactly one other seated player's ships and no neutral ones (`mahact_units::hero_pairs`).
fn mahact_pairs(sx: &Sx<'_>, who: Seat) -> Vec<(System, System)> {
    use crate::rules::combat::{combatants, is_neutral, ships_of, Rd};
    let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
    let origins: Vec<System> = sx.state.board().systems().map(|(system, _)| system).filter(|system| crate::query::leader_reads::has_entry(sx.state, *system)).collect();
    let mut pairs: Vec<(System, System)> = Vec::new();
    for origin in origins {
        if ships_of(rd, who, origin).is_empty() {
            continue;
        }
        let Some(near) = board::neighbours(sx.state, origin) else { continue };
        for index in near.iter() {
            let Some(destination) = System::ALL.get(index).copied() else { continue };
            let others: Vec<Seat> = combatants(rd, destination).as_slice().iter().copied().filter(|side| *side != who).collect();
            if let [other] = others.as_slice()
                && !is_neutral(sx.state, *other)
                && !pairs.contains(&(origin, destination))
            {
                pairs.push((origin, destination));
            }
        }
    }
    pairs
}

/// What the fight of one pair needs of the position (`dual_window::mahact_hero`'s preparation): the position the fight starts from (the fleet moved,
/// the destination the active system, the barring mark set, the two sides written), the limits of the enforcement there, the environment of the machine
/// and the fields of a fresh window, and the notes held now.
fn mahact_prepared(sx: &Sx<'_>, who: Seat, from: System, to: System) -> Option<crate::rules::combat::mahact_hero::Prepared> {
    use crate::keys::{Atom, MarkKey, MarkVal};
    use crate::rules::combat::{combatants, Rd};
    use crate::state::{CombatFields, CombatStep, HitPhase};
    let mut sim = sx.state.clone();
    let fleet: Vec<Piece> = sim.board().sys(from).units.as_slice().iter().copied().filter(|p| p.owner() == who).collect();
    for piece in fleet {
        crate::kernel::fx_move::move_ship(&mut sim, from, to, piece);
    }
    sim.meta_mut().active_system = Some(to);
    sim.marks_mut().tail.edit().insert(MarkKey::MahactBenediction(), MarkVal::Atom(Atom::System(to)));
    let rd = Rd::new(&sim, sx.kernel).with_blanked(reach::blanked(&sim));
    let defender = combatants(rd, to).as_slice().iter().copied().find(|side| *side != who)?;
    sim.transient_mut().combat.active_space_combat = Some((to, who, defender));
    let limits = crate::driver::windows::tactical::limits(&sim, sx.kernel, who, to);
    let fields = CombatFields {
        step: CombatStep::Opening { round: 1 },
        phase: HitPhase::Round,
        resuming: false,
        pending_retreats: Vec::new(),
        offered: Vec::new(),
        damaged: Vec::new(),
        fled: Vec::new(),
        at_start: Vec::new(),
        scoring: false, // mutants:skip: the machine writes the flag itself when a barrage's feat opens a scoring window and reads it only after
        outcome: None,
        end_announced: false,
        round_end_produced: None,
    };
    let env = crate::driver::windows::combat::env_of(&sim, sx.kernel, &sx.table.facts.combat, &fields, 0)?;
    let before = crate::state::Before { sides: crate::containers::SmallList::new(), capitals: crate::containers::SmallList::new(), notes: crate::driver::windows::tactical::note_holdings(&sim) };
    Some(crate::rules::combat::mahact_hero::Prepared { from, to, limits: SeeingLimits { enforce: limits.enforce, shown: limits.shown, halves: limits.halves }, env, fields, before })
}

/// Airo Shir Aur (Mahact hero, an ACTION): the fleet of the seat in one system moves to an adjacent system that holds exactly one other player's ships,
/// the destination is enforced and the space combat is fought there (neither side may retreat). Purged when the card resolved.
pub fn airo_shir_aur(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::rules::combat::machine::Bail;
    let pairs = mahact_pairs(sx, who);
    let mut prepared = Vec::new();
    for (from, to) in &pairs {
        let Some(one) = mahact_prepared(sx, who, *from, *to) else { return gap_flow(sx, TurnGap::NoLink) };
        prepared.push(one);
    }
    let mut window = crate::rules::combat::mahact_hero::MahactHero::new(who, pairs, prepared);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if let Some(why) = window.bailed {
        let gap = match why {
            Bail::RollEndsFight => crate::kernel::ability::AftermathGap::RollEndsFight,
            Bail::Unported => crate::kernel::ability::AftermathGap::CombatUnported,
        };
        if sx.tape.unknown.get().is_none() { // mutants:skip: the machine's own stop; a gap noted earlier is kept
            sx.tape.unknown.set(Some(crate::kernel::ability::Unknown::UnportedAftermath(gap)));
        }
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::HeroFight);
    }
    finish(sx, who, key, Leader::AiroShirAur, window.used)
}

/// Dannel of the Tenth (Yin hero): every planet that is not in a home system, in the order of the hexes of the map (`yin::hero_spots`: the string's
/// `Galaxy::system_ids`, by `q` then `r`, not the order of the system table). The Wormhole Nexus is off the grid (user ruling 2026-11-04): its locked
/// planet is never a target, Mallice is one once the Nexus is open, and it comes last, after the planets on the grid.
fn yin_spots(sx: &Sx<'_>) -> Vec<(System, crate::generated::Planet)> {
    use crate::generated::Planet;
    let Some(galaxy) = sx.state.galaxy() else { return Vec::new() };
    let mut spots = Vec::new();
    for system in galaxy.placed_in_hex_order() {
        let Some(system) = System::ALL.get(system.index()).copied() else { continue };
        if system.data().home_system {
            continue;
        }
        spots.extend(system.data().planets.iter().filter(|planet| !matches!(**planet, Planet::Mallice | Planet::LockedMallice)).map(|planet| (system, *planet)));
    }
    // (The spot names the tile that sits beside the map, as the ships' space area does; the planet's own entries are kept under the system it is printed in:
    // `YinHero` touches that one, see `yin_hero::home`.)
    let nexus = System::MalliceLockedNexus;
    if sx.state.meta().flags.has(crate::state::MetaFlags::NEXUS_UNLOCKED) && galaxy.in_play().contains(nexus.index()) {
        spots.push((nexus, Planet::Mallice));
    }
    spots
}

/// The positions of Thunder's Edge content the commitment window does not carry (a neutral side, Proxima Targeting VI, Alarum, L1Z1X's Fealty
/// Uplink, a coexisting planet): the counted gap [`TurnGap::HeroFight`].
fn yin_unported(sx: &Sx<'_>, who: Seat, spots: &[(System, crate::generated::Planet)]) -> bool {
    let everyone: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    if sx.kernel.hooks.direct.bastion_units_watches_ground_round(sx.state, &everyone) {
        return true; // mutants:skip: Proxima Targeting VI is a technology of the dropped faction Last Bastion
    }
    if sx.state.players().by_seat(who).is_some_and(|p| p.cards.breakthrough == Some(crate::generated::Breakthrough::FealtyUplink)) {
        return true;
    }
    spots.iter().any(|(system, planet)| {
        sx.kernel.hooks.direct.ralnel_alarum_on(sx.state, *system, *planet) // mutants:skip: Alarum is the mech of the dropped faction Ral Nel
            || crate::query::board::in_coexistence(sx.state, *system, *planet)
            || sx.state.board().planet(*planet).units.as_slice().iter().any(|p| crate::rules::combat::is_neutral(sx.state, p.owner()))
    })
}

/// Dannel of the Tenth (Yin hero, an ACTION): up to three infantry from the reinforcements are committed to planets outside the home systems and each
/// planet chosen is fought to the end. Purged when some infantry was committed (the card resolved).
pub fn dannel(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::rules::invasion::fight::FightFacts;
    use crate::rules::invasion::yin_hero::{YinFacts, YinHero};
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let Some(infantry) = crate::kernel::fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry) else { return gap_flow(sx, TurnGap::NoRoster) };
    let stock = crate::query::fleet::remaining_all(sx.state, scope, who, infantry).min(3);
    let stock = stock.max(0); // mutants:skip: a box holding none is the early verdict below, which the menu never reaches
    let spots = yin_spots(sx);
    if stock == 0 || spots.is_empty() { // mutants:skip: the menu lists the hero only with infantry in the box on a mapped board; the string's early verdict is its defensive twin
        return finish(sx, who, key, Leader::DannelOfTheTenth, false); // mutants:skip: as above
    }
    if yin_unported(sx, who, &spots) {
        return gap_flow(sx, TurnGap::HeroFight);
    }
    let codes = sx.table.codes.invasion;
    let owns = |seat: Seat, tech: Technology| sx.state.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(tech));
    let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    let mut dacxive = Vec::new();
    for seat in &seats {
        if owns(*seat, Technology::DacxiveAnimators) { // mutants:skip: the placement after a won combat needs dice that win; the fight's own tests (`invasion_windows`) pin Dacxive Animators
            let Some(form) = crate::kernel::fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, *seat, UnitBaseType::Infantry) else { return gap_flow(sx, TurnGap::NoRoster) };
            dacxive.push((*seat, form));
        }
    }
    let fight = FightFacts {
        invader: who,
        system: spots[0].0,
        planet: spots[0].1,
        order: seats.clone(),
        announce: true,
        copied: crate::driver::windows::invasion::InvasionMachine::copied_round(sx.state),
        numbered: sx.state.meta().rules_at_least(16), // mutants:skip: only the latest rules version is played, which is past 16 and 17
        can_pay: [false, false], // mutants:skip: `pay_env` is set, so the round reads what each seat can spend and `can_pay` stands for no round
        pay_env: Some(crate::kernel::fx_pay::PayEnv { scope: crate::tables::Scope(u32::from(sx.kernel.sources.0)), blanked: reach::blanked(sx.state) }), // mutants:skip: as above (the Reaper's payment of a Letnev rival)
        dunlain: codes.dunlain_reaper,
        kind: codes.ground,
        names: codes.names.to_vec(),
        unit_codes: crate::generated::Unit::ALL.iter().map(|u| (*u, codes.unit(*u))).collect(),
        cause: sx.table.codes.ground_combat,
        harrow_cause: sx.table.codes.harrow,
        step_kind: codes.ground_round,
        harrow: sx.state.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Harrow)),
        x89: seats.iter().copied().filter(|seat| crate::query::tech::holds_x89(sx.state, *seat)).collect(),
        dacxive,
        notes: crate::driver::windows::tactical::note_holdings(sx.state).as_slice().to_vec(),
    };
    let facts = YinFacts { spots, stock: u8::try_from(stock).unwrap_or(3), // mutants:skip: the stock was clamped to 0..=3 above
        infantry, fight, minister: codes.minister };
    let mut window = YinHero::new(who, facts);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::HeroFight);
    }
    finish(sx, who, key, Leader::DannelOfTheTenth, window.used)
}
