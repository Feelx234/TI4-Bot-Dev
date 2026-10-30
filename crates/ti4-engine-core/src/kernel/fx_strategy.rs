//! `fx_strategy`: the writes of the strategy area that more than one rule area needs (package B8c-2): the waivers of a
//! secondary (a note handed over, an agent exhausted), the writes a research ends in, and the unit forms an upgrade
//! changes. Nothing here asks a question; nothing calls a rule area (the layer rule: `rules::*` may not import each
//! other, so the writes of `rules::trade` and `rules::leaders` that the strategy windows need are repeated here).

use crate::generated::{Agenda, Leader, Planet, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::query::tech::UnitRoster;
use crate::query::{laws, promissory as qn, tech};
use crate::state::{CoreState, LeaderStatus, NoteKey, Piece};
use crate::tables::Scope;

/// `promissory::take`: `holder` gets the note, faceup if that is where the card lives (69.3). The same write as
/// `rules::trade::take_note`.
pub fn take_note(core: &mut CoreState, holder: Seat, key: NoteKey) {
    core.cards_mut().promissory.edit().insert(key, holder);
    if qn::is_play_area(key.note, key.faction)
        && !qn::is_action_placed(core, key.note)
        && key.note != crate::generated::PromissoryNote::Antivirus
    {
        core.cards_mut().promissory_faceup.edit().insert(key, ());
    }
}

/// `leaders::exhaust` without the Nomad's watch: a readied leader of `who` becomes exhausted; `false` when it was not
/// readied (or the seat does not hold it).
pub fn exhaust_leader(core: &mut CoreState, who: Seat, leader: Leader) -> bool {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    match p.leaders.status.get_mut(&leader) {
        Some(status) if *status == LeaderStatus::Readied => {
            *status = LeaderStatus::Exhausted;
            true
        }
        _ => false,
    }
}

// -- research ---------------------------------------------------------------------------------------------

/// `technology::exhaust` of an owned technology card (Inheritance Systems, AI Development Algorithm): whether it was
/// owned and ready.
pub fn exhaust_technology(core: &mut CoreState, who: Seat, tech: Technology) -> bool {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    p.tech.owned.contains(tech) && p.tech.exhausted.insert(tech)
}

/// `technology::unit_swaps`: for each unit form in scope the form `who` is entitled to instead (the upgrade owned, else
/// the faction's own unit for a plain one, then the faction override of the hook table). Only the forms that change.
#[must_use]
pub fn unit_swaps(core: &CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat) -> Vec<(Unit, Unit)> {
    let Some(p) = core.players().by_seat(who) else { return Vec::new() };
    let upgrade: Vec<Option<Unit>> =
        UnitBaseType::ALL.iter().map(|b| tech::unlocked_upgrade(scope, *b, p.faction, &p.tech.owned)).collect();
    let mut swaps = Vec::new();
    for kind in Unit::ALL {
        let Some(d) = scope.unit(kind) else { continue };
        let base = d.base_type;
        let own = if d.faction.is_none() && matches!(d.required_technology, crate::generated::Requirement::None) {
            tech::faction_unit(roster, p.faction, base)
        } else {
            None
        };
        let chosen = upgrade[base as usize].or(own).unwrap_or(kind);
        let chosen = k.hooks.strategy.unit_form_override(core, k, who, base, chosen).unwrap_or(chosen);
        if chosen != kind {
            swaps.push((kind, chosen));
        }
    }
    swaps
}

/// `technology::apply_unit_upgrades` (90.8): every unit of `who` on the board changes to the form the technologies
/// owned entitle it to, in space and on planets, keeping its place in the list and its damage.
pub fn apply_unit_upgrades(core: &mut CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat) {
    let swaps = unit_swaps(core, k, scope, roster, who);
    if swaps.is_empty() {
        return;
    }
    let swapped = |piece: Piece| -> Option<Piece> {
        if piece.owner() != who {
            return None;
        }
        swaps.iter().find(|(from, _)| *from == piece.kind()).map(|(_, to)| piece.with_kind(*to))
    };
    for system in System::ALL {
        if core.board().sys(system).units.as_slice().iter().any(|u| swapped(*u).is_some()) {
            for piece in core.board_mut().sys_mut(system).units.as_mut_slice() {
                if let Some(better) = swapped(*piece) {
                    *piece = better;
                }
            }
        }
    }
    for planet in Planet::ALL {
        if core.board().planet(planet).units.as_slice().iter().any(|u| swapped(*u).is_some()) {
            for piece in core.board_mut().planet_mut(planet).units.as_mut_slice() {
                if let Some(better) = swapped(*piece) {
                    *piece = better;
                }
            }
        }
    }
}

/// `laws::revolution_tax`: with the Anti-Intellectual Revolution elected For, the cheapest non-fighter ship of `who` in
/// space is destroyed after a research (the first of the cheapest in system order, then list order). Returns its system.
pub fn revolution_tax(core: &mut CoreState, scope: Scope, blanked: bool, who: Seat) -> Option<System> {
    if !laws::active(core, blanked, Agenda::AntiIntellectualRevolution)
        || laws::elected(core, blanked, Agenda::AntiIntellectualRevolution) != Some(crate::keys::Outcome::For)
    {
        return None;
    }
    let mut cheapest: Option<(i64, System, usize)> = None;
    for system in System::ALL {
        for (at, piece) in core.board().sys(system).units.as_slice().iter().enumerate() {
            if piece.owner() != who {
                continue;
            }
            let Some(d) = scope.unit(piece.kind()) else { continue };
            if !d.ship || d.base_type == UnitBaseType::Fighter {
                continue;
            }
            // `production::price`: a fraction of a resource is one (the single-unit law does not touch the cost).
            let halves = i64::from(d.cost_halves.unwrap_or(0));
            let cost = if halves == 1 { 1 } else { ((halves + 1) / 2).clamp(0, 64) };
            if cheapest.is_none_or(|(best, _, _)| cost < best) {
                cheapest = Some((cost, system, at));
            }
        }
    }
    let (_, system, at) = cheapest?;
    core.board_mut().sys_mut(system).units.remove(at);
    Some(system)
}

/// `nekro::note_propagation`: a research the Nekro would have made became three command tokens; the game opens the token
/// window at its next step.
pub fn note_propagation(core: &mut CoreState, who: Seat) {
    let key = MarkKey::NekroPropagation(who);
    let pending = match core.marks().tail.get(&key) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    };
    core.marks_mut().tail.edit().insert(key, MarkVal::Atom(Atom::Num(pending.saturating_add(1))));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    fn game() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        s
    }

    fn scope() -> Scope {
        Scope(u32::MAX)
    }

    /// Regression (mutation `revolution taxes fighters`): the fighters come first in the list and cost as little as the
    /// destroyer, but the tax takes a ship that is no fighter.
    #[test]
    fn the_revolution_takes_the_cheapest_ship_that_is_no_fighter() {
        let mut s = game();
        s.agenda_mut().laws.insert(Agenda::AntiIntellectualRevolution, crate::keys::Outcome::For);
        let system = System::ALL[4];
        for unit in [Unit::FighterI, Unit::FighterI, Unit::DestroyerI, Unit::CruiserI] {
            s.board_mut().sys_mut(system).units.push(Piece::new(unit, Seat(1)));
        }
        assert_eq!(revolution_tax(&mut s, scope(), false, Seat(1)), Some(system));
        let left: Vec<Unit> = s.board().sys(system).units.as_slice().iter().map(|p| p.kind()).collect();
        assert_eq!(left, vec![Unit::FighterI, Unit::FighterI, Unit::CruiserI]);
        // Against, or no law at all, taxes nothing; a seat with only fighters pays nothing.
        s.agenda_mut().laws.insert(Agenda::AntiIntellectualRevolution, crate::keys::Outcome::Against);
        assert_eq!(revolution_tax(&mut s, scope(), false, Seat(1)), None);
    }

    /// Regression (mutation `with_kind drops owner and damage`, seen through the board write).
    #[test]
    fn an_upgrade_keeps_the_owner_and_the_damage_of_the_unit_on_the_board() {
        let mut s = game();
        let (system, planet) = (System::ALL[4], crate::generated::Planet::ALL[4]);
        s.players_mut().by_seat_mut(Seat(2)).unwrap().tech.owned.insert(Technology::DestroyerII);
        let before = Piece::new(Unit::DestroyerI, Seat(2)).with_sustained(true);
        s.board_mut().sys_mut(system).units.push(before);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::DestroyerI, Seat(1)));
        let _ = planet;
        let k = Kernel::plain();
        apply_unit_upgrades(&mut s, &k, scope(), &UnitRoster::default(), Seat(2));
        let units = s.board().sys(system).units.as_slice().to_vec();
        assert_eq!(units[0].kind(), Unit::DestroyerII);
        assert_eq!((units[0].owner(), units[0].sustained()), (Seat(2), true));
        assert_eq!(units[1].kind(), Unit::DestroyerI, "another owner's unit is not touched");
    }
}

// -- unit forms (package W3: the unit-form flip) -----------------------------------------------------------

/// `action_cards::placed_unit_id` with the faction override: the form a seat places for a base type (the upgrade it owns,
/// else its faction's own unit, else the plain one; then the override of the hook table, Mentak's Corsair).
#[must_use]
pub fn placed_unit(core: &CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat, base: UnitBaseType) -> Option<Unit> {
    let form = tech::unit_form(core, scope, roster, who, base)?;
    Some(k.hooks.strategy.unit_form_override(core, k, who, base, form).unwrap_or(form))
}

/// `exploration::place_on_planet` (and the placing effects that go through the one door, 31.4): the seat's own form for `base`
/// (`placed_unit`), refused when a faction effect forbids the placement or the reinforcements hold none, put on `planet` (or in the
/// space area of `system`), and a Naaz mech announced. The unit placed; `None` with nothing changed.
pub fn place_for(
    core: &mut CoreState,
    k: &Kernel,
    scope: Scope,
    roster: &UnitRoster,
    who: Seat,
    system: System,
    planet: Option<Planet>,
    base: UnitBaseType,
) -> Option<Unit> {
    let unit = placed_unit(core, k, scope, roster, who, base)?;
    if k.hooks.economy.effect_placement_forbidden(core, k, who, unit) || crate::query::fleet::remaining_all(core, scope, who, unit) <= 0 {
        return None;
    }
    crate::kernel::fx::place_unit(core, who, system, planet, unit);
    crate::kernel::fx::stage_naaz_mech_placed(core, who, system, unit);
    Some(unit)
}

/// `fleet::flip_form`: flip one unit of `who` of kind `from` to its other form, in place: in `system`'s space area when
/// `planet` is `None`, on that planet otherwise. The first unit of that owner and kind in the list is the one (damage
/// and galvanize stay with the piece). `None` with nothing changed: no twin, a flip to a ship form on a planet, or no
/// such unit. Both forms are the same plastic, so no supply changes.
pub fn flip_form(core: &mut CoreState, scope: Scope, who: Seat, system: System, planet: Option<Planet>, from: Unit) -> Option<Unit> {
    let to = tech::other_form(from).filter(|t| scope.unit(*t).is_some())?;
    if planet.is_some() && scope.unit(to).is_some_and(|d| d.ship) {
        return None;
    }
    let pool = match planet {
        Some(p) => core.board_mut().planet_mut(p).units.as_mut_slice(),
        None => core.board_mut().sys_mut(system).units.as_mut_slice(),
    };
    let piece = pool.iter_mut().find(|u| u.owner() == who && u.kind() == from)?;
    *piece = piece.with_kind(to);
    Some(to)
}

/// `fleet::flip_to_ship_forms` (`to_ship`: "at the start of a space combat", the Naaz Eidolon) and `flip_to_ground_forms`
/// (`!to_ship`: "at the end of a space battle", the Z-Grav Eidolon): every dual-form unit of `players` in the space area
/// of `system` whose form is not (is) a ship takes its other form. Returns each flip as `(owner, new form)` in list order.
pub fn flip_space_area(core: &mut CoreState, scope: Scope, players: &[Seat], system: System, to_ship: bool) -> Vec<(Seat, Unit)> {
    let ship = |u: Unit| scope.unit(u).map(|d| d.ship);
    let mut flipped = Vec::new();
    for piece in core.board_mut().sys_mut(system).units.as_mut_slice() {
        if !players.contains(&piece.owner()) || ship(piece.kind()) != Some(!to_ship) {
            continue;
        }
        let Some(twin) = tech::other_form(piece.kind()).filter(|t| ship(*t) == Some(to_ship)) else { continue };
        *piece = piece.with_kind(twin);
        flipped.push((piece.owner(), twin));
    }
    flipped
}

#[cfg(test)]
mod form_tests {
    use super::*;
    use crate::state::Piece;

    fn scope() -> Scope {
        Scope(u32::MAX)
    }

    #[test]
    fn the_eidolon_and_the_z_grav_eidolon_are_twins_and_nothing_else_is() {
        assert_eq!(tech::other_form(Unit::Eidolon), Some(Unit::ZGravEidolonNaazMechSpace));
        assert_eq!(tech::other_form(Unit::ZGravEidolonNaazMechSpace), Some(Unit::Eidolon));
        for u in Unit::ALL.into_iter().filter(|u| !matches!(u, Unit::Eidolon | Unit::ZGravEidolonNaazMechSpace)) {
            assert_eq!(tech::other_form(u), None, "{u:?}");
        }
        // the twins are the same plastic: one base type, one faction
        let (a, b) = (Unit::Eidolon.data(), Unit::ZGravEidolonNaazMechSpace.data());
        assert!(a.base_type == b.base_type && a.faction == b.faction && !a.ship && b.ship);
    }

    #[test]
    fn a_flip_changes_the_first_unit_of_the_kind_and_keeps_its_damage() {
        let mut s = CoreState::all_loaded();
        let (sys, planet) = (System::ALL[3], Planet::ALL[3]);
        let who = Seat(0);
        let damaged = Piece::new(Unit::Eidolon, who).with_sustained(true);
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::Eidolon, Seat(1)));
        s.board_mut().sys_mut(sys).units.push(damaged);
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::Eidolon, who));
        assert_eq!(flip_form(&mut s, scope(), who, sys, None, Unit::Eidolon), Some(Unit::ZGravEidolonNaazMechSpace));
        let kinds: Vec<_> = s.board().sys(sys).units.iter().map(|p| (p.owner(), p.kind(), p.sustained())).collect();
        assert_eq!(kinds, [(Seat(1), Unit::Eidolon, false), (who, Unit::ZGravEidolonNaazMechSpace, true), (who, Unit::Eidolon, false)]);
        // a ship form is never on a planet; a unit without a twin or without a unit changes nothing
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::ZGravEidolonNaazMechSpace, who));
        assert_eq!(flip_form(&mut s, scope(), who, sys, Some(planet), Unit::ZGravEidolonNaazMechSpace), Some(Unit::Eidolon), "a flip to the ground form is legal on a planet");
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::Eidolon, who));
        assert_eq!(flip_form(&mut s, scope(), who, sys, Some(planet), Unit::Eidolon), None, "to a ship form on a planet is refused");
        assert_eq!(flip_form(&mut s, scope(), who, sys, None, Unit::InfantryI), None);
        assert_eq!(flip_form(&mut s, scope(), Seat(2), sys, None, Unit::Eidolon), None);
    }

    #[test]
    fn the_space_area_flips_every_matching_unit_of_the_players_only() {
        let mut s = CoreState::all_loaded();
        let sys = System::ALL[4];
        let (a, b) = (Seat(0), Seat(1));
        for p in [Piece::new(Unit::Eidolon, a), Piece::new(Unit::Eidolon, b), Piece::new(Unit::ZGravEidolonNaazMechSpace, a), Piece::new(Unit::CruiserI, a)] {
            s.board_mut().sys_mut(sys).units.push(p);
        }
        // start of a combat: only a's ground-form mech becomes a ship
        assert_eq!(flip_space_area(&mut s, scope(), &[a], sys, true), [(a, Unit::ZGravEidolonNaazMechSpace)]);
        let kinds: Vec<_> = s.board().sys(sys).units.iter().map(|p| p.kind()).collect();
        assert_eq!(kinds, [Unit::ZGravEidolonNaazMechSpace, Unit::Eidolon, Unit::ZGravEidolonNaazMechSpace, Unit::CruiserI]);
        // end of the battle: both of a's ship forms go back
        assert_eq!(flip_space_area(&mut s, scope(), &[a], sys, false), [(a, Unit::Eidolon), (a, Unit::Eidolon)]);
        assert_eq!(flip_space_area(&mut s, scope(), &[a], sys, false), []);
    }
}
