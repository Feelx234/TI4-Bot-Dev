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
